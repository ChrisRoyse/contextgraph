use clap::{Parser, Subcommand};
use context_graph_mejepa_corpus::multilang_fixture_corpus::{
    build_fixture_tasks, non_python_languages, sha256_text as fixture_sha256_text,
    validate_toolchain, FixtureTask,
};
use context_graph_mejepa_corpus::oracle::{
    OracleStore, CF_MEJEPA_CORPUS_MUTATION_OUTCOMES, CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON,
    CF_MEJEPA_CORPUS_PROVENANCE_JSON,
};
use context_graph_mejepa_corpus::patch_mutation::PatchMutationOutcome;
use context_graph_mejepa_corpus::source_patch::{
    mutate_task_source_patch, SourcePatchConfig, SourcePatchTask,
};
use context_graph_mejepa_corpus::split::{
    assert_no_test_patch_leakage, split_corpus, CorpusEntry, SplitConfig, SplitMode, TaskAssignment,
};
use context_graph_mejepa_corpus::swebench::{
    normalize_swebench_oracle_verdict, run_swebench_lite_oracle, run_swebench_lite_oracle_batch,
    SwebenchBatchOracleConfig, SwebenchOracleConfig, SwebenchPrediction, SwebenchPredictionMode,
};
use context_graph_mejepa_corpus::timed_subprocess::run_capture_timed;
use context_graph_mejepa_corpus::{
    apply_mutation_for_language, compute_corpus_sha256, parse_languages, CorpusProvenance,
    CorpusProvenanceRow, Language, MutationCategory, MutationConfig, MutationOutcome,
    MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
};
use context_graph_mejepa_instruments::{
    ExceptionClass, OracleVerdict, PerTestOutcome, TestOutcome,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

type CliResult<T> = Result<T, Box<dyn std::error::Error>>;
const LOG_TAIL_BYTES: u64 = 64 * 1024;
const DOCKER_QUERY_TIMEOUT: Duration = Duration::from_secs(120);
const OFFICIAL_PATCH_LOADER_TIMEOUT: Duration = Duration::from_secs(300);
const AIWONDER_SWEBENCH_PYTHON: &str = "/zfs/hot/contextgraph/venv/bin/python3";

#[derive(Parser, Debug)]
#[command(version, about = "ME-JEPA Phase 0 corpus generation and verification")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand, Debug)]
enum CommandKind {
    Generate(GenerateArgs),
    Verify(VerifyArgs),
    Stats(StatsArgs),
    Smoke(SmokeArgs),
    Merge(MergeArgs),
    Normalize(NormalizeArgs),
    PruneQuarantine(PruneQuarantineArgs),
    GenerateFixtures(GenerateFixturesArgs),
    PhaseCVerify(PhaseCVerifyArgs),
    PrepareImages(PrepareImagesArgs),
    Cleanup(CleanupArgs),
}

#[derive(Parser, Debug, Clone)]
struct GenerateArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(long, default_value = "princeton-nlp/SWE-bench_Lite")]
    dataset_name: String,
    #[arg(long, default_value = "test")]
    dataset_split: String,
    #[arg(
        long,
        default_value = AIWONDER_SWEBENCH_PYTHON
    )]
    venv_python: PathBuf,
    #[arg(long = "instance-id")]
    instance_ids: Vec<String>,
    #[arg(long = "category")]
    categories: Vec<String>,
    #[arg(long = "language", value_delimiter = ',', default_value = "python")]
    languages: Vec<String>,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    #[arg(long)]
    max_tasks: Option<usize>,
    #[arg(long, default_value_t = 0)]
    skip_tasks: usize,
    // EPHEMERAL — defaults pinned to ext4 (~/.cache/...) for ~40-100× WSL2
    // throughput vs cross-filesystem mounted drives. On native Linux ~/.cache/ is still ext4
    // and Just Works. The require_ephemeral_path validator forbids C-drive
    // mounts; durable production data is kept under /zfs/archive/contextgraph.
    #[arg(long, default_value = "/home/cabdru/.cache/mejepa-corpus/repos")]
    repo_cache_dir: PathBuf,
    #[arg(long, default_value = "/home/cabdru/.cache/mejepa-corpus/work")]
    source_work_root: PathBuf,
    #[arg(long, default_value = "mejepa-corpus")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
    #[arg(long, default_value_t = 4)]
    oracle_workers: usize,
    /// Rayon thread pool size for parallel patch generation. 0 = auto
    /// (num_cpus). Bottleneck on a single core was ~10 s/task; parallel
    /// across the AMD 9950X3D's 16 cores brings it to <1 s/task.
    #[arg(long, default_value_t = 0)]
    patch_workers: usize,
    #[arg(long, default_value_t = false)]
    resume_incomplete: bool,
}

#[derive(Parser, Debug, Clone)]
struct VerifyArgs {
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long, default_value_t = 0.05)]
    sample_fraction: f64,
    #[arg(long = "sample-key")]
    sample_keys: Vec<String>,
    #[arg(long)]
    quarantine_config: Option<PathBuf>,
    #[arg(
        long,
        default_value = AIWONDER_SWEBENCH_PYTHON
    )]
    venv_python: PathBuf,
    #[arg(long, default_value = "mejepa-corpus-verify")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
    #[arg(long, default_value_t = 1)]
    oracle_repeat_runs: usize,
    #[arg(long)]
    oracle_repeat_output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusQuarantineConfig {
    #[serde(default)]
    quarantined_tasks: Vec<CorpusQuarantineEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusQuarantineEntry {
    task_id: String,
    reason: String,
    flakiness_rate: f64,
    oracle_runs: usize,
    observed_outcomes: Vec<bool>,
    observed_verdict_sha256: Vec<String>,
    operator_id: String,
    created_unix_ms: i64,
}

#[derive(Parser, Debug, Clone)]
struct StatsArgs {
    #[arg(long)]
    corpus: PathBuf,
}

#[derive(Parser, Debug, Clone)]
struct SmokeArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(long, default_value = "princeton-nlp/SWE-bench_Lite")]
    dataset_name: String,
    #[arg(long, default_value = "test")]
    dataset_split: String,
    #[arg(
        long,
        default_value = AIWONDER_SWEBENCH_PYTHON
    )]
    venv_python: PathBuf,
    #[arg(long = "instance-id")]
    instance_ids: Vec<String>,
    #[arg(long = "category")]
    categories: Vec<String>,
    #[arg(long = "language", value_delimiter = ',', default_value = "python")]
    languages: Vec<String>,
    #[arg(long, default_value_t = 0)]
    seed: u64,
    #[arg(long)]
    max_tasks: Option<usize>,
    #[arg(long, default_value_t = 0)]
    skip_tasks: usize,
    // EPHEMERAL — defaults pinned to ext4 (~/.cache/...) for ~40-100× WSL2
    // throughput vs cross-filesystem mounted drives. On native Linux ~/.cache/ is still ext4
    // and Just Works. The require_ephemeral_path validator forbids C-drive
    // mounts; durable production data is kept under /zfs/archive/contextgraph.
    #[arg(long, default_value = "/home/cabdru/.cache/mejepa-corpus/repos")]
    repo_cache_dir: PathBuf,
    #[arg(long, default_value = "/home/cabdru/.cache/mejepa-corpus/work")]
    source_work_root: PathBuf,
    #[arg(long, default_value = "mejepa-corpus-smoke")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
    #[arg(long, default_value_t = 4)]
    oracle_workers: usize,
    #[arg(long, default_value_t = false)]
    resume_incomplete: bool,
    #[arg(long, default_value_t = 0.25)]
    sample_fraction: f64,
    #[arg(long, default_value_t = 900.0)]
    max_generate_secs_per_entry: f64,
    #[arg(long, default_value_t = 900.0)]
    max_verify_secs_per_sample: f64,
}

#[derive(Parser, Debug, Clone)]
struct MergeArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long = "shard")]
    shards: Vec<PathBuf>,
    #[arg(long, default_value_t = false)]
    allow_partial: bool,
}

#[derive(Parser, Debug, Clone)]
struct NormalizeArgs {
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Parser, Debug, Clone)]
struct PruneQuarantineArgs {
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long)]
    quarantine_config: PathBuf,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Parser, Debug, Clone)]
struct GenerateFixturesArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value_t = 64)]
    tasks_per_language: usize,
    #[arg(long, default_value_t = 17)]
    seed: u64,
    #[arg(long, default_value_t = 45)]
    tool_timeout_secs: u64,
}

#[derive(Parser, Debug, Clone)]
struct PhaseCVerifyArgs {
    #[arg(long)]
    python_corpus: PathBuf,
    #[arg(long)]
    non_python_corpus: PathBuf,
    #[arg(
        long,
        default_value = "/zfs/archive/contextgraph/fsv/corpus-fsv/split_full.json"
    )]
    evidence: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(long, default_value_t = 500)]
    min_entries_per_non_python_language: usize,
}

#[derive(Parser, Debug, Clone)]
struct PrepareImagesArgs {
    #[arg(long)]
    evidence: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(long = "instance-id")]
    instance_ids: Vec<String>,
    #[arg(long)]
    max_tasks: Option<usize>,
    #[arg(long, default_value_t = 0)]
    skip_tasks: usize,
    #[arg(long, default_value = "swebench")]
    namespace: String,
    #[arg(long, default_value_t = 6)]
    parallel: usize,
    #[arg(long, default_value_t = 1800)]
    pull_timeout_secs: u64,
}

#[derive(Parser, Debug, Clone)]
struct CleanupArgs {
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long, default_value = "swebench")]
    namespace: String,
    #[arg(long, default_value_t = false)]
    execute: bool,
    #[arg(long, default_value_t = false)]
    remove_containers: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct TaskManifest {
    instance_id: String,
    repo: String,
    base_commit: String,
    official_test_patch_sha256: String,
    oracle_patch_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusIndex {
    schema_version: u32,
    #[serde(default = "default_corpus_version")]
    corpus_version: String,
    #[serde(default = "default_embedder_version")]
    embedder_version: String,
    #[serde(default = "default_dataset_name")]
    dataset_name: String,
    #[serde(default = "default_dataset_split")]
    dataset_split: String,
    #[serde(default)]
    complete: bool,
    generated_at_unix_ms: u128,
    #[serde(default)]
    seed: u64,
    #[serde(default)]
    languages: Vec<Language>,
    tasks_dir: String,
    store_path: String,
    #[serde(default)]
    corpus_sha256: Option<String>,
    #[serde(default)]
    selected_task_ids: Vec<String>,
    entries: Vec<GeneratedEntry>,
    split_assignments: Vec<TaskAssignment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedEntry {
    task_id: String,
    repo: String,
    #[serde(default = "default_entry_language")]
    language: Language,
    category: MutationCategory,
    bucket: String,
    patch_path: String,
    mutation_note: String,
    patch_sha256: String,
    oracle_verdict_sha256: String,
    oracle_all_passed: bool,
    oracle_exception: Option<String>,
    oracle_per_test_count: usize,
}

fn default_entry_language() -> Language {
    Language::Python
}

fn default_dataset_name() -> String {
    "princeton-nlp/SWE-bench_Lite".to_string()
}

fn default_dataset_split() -> String {
    "test".to_string()
}

fn main() -> CliResult<()> {
    match Cli::parse().command {
        CommandKind::Generate(args) => generate(args),
        CommandKind::Verify(args) => verify(args),
        CommandKind::Stats(args) => stats(args),
        CommandKind::Smoke(args) => smoke(args),
        CommandKind::Merge(args) => merge(args),
        CommandKind::Normalize(args) => normalize(args),
        CommandKind::PruneQuarantine(args) => prune_quarantine(args),
        CommandKind::GenerateFixtures(args) => generate_fixtures(args),
        CommandKind::PhaseCVerify(args) => phase_c_verify(args),
        CommandKind::PrepareImages(args) => prepare_images(args),
        CommandKind::Cleanup(args) => cleanup(args),
    }
}

fn generate(mut args: GenerateArgs) -> CliResult<()> {
    args.output = require_corpus_path(&args.output, "generate.output")?;
    args.venv_python = require_runtime_python_path(&args.venv_python, "generate.venv_python")?;
    // Cache + source-work are ephemeral: rebuildable from durable inputs
    // and pinned to local scratch for throughput before durable aiwonder promotion.
    args.repo_cache_dir = require_ephemeral_path(&args.repo_cache_dir, "generate.repo_cache_dir")?;
    args.source_work_root =
        require_ephemeral_path(&args.source_work_root, "generate.source_work_root")?;
    validate_generate_args(&args)?;
    let categories = parse_categories(&args.categories)?;
    let languages =
        parse_languages(&args.languages).map_err(|err| format!("{err} ({})", err.code()))?;
    if args.resume_incomplete {
        ensure_resume_directory_ready(&args.output)?;
    } else {
        ensure_output_directory_ready(&args.output)?;
    }
    let interrupted = install_shutdown_flag()?;
    let manifests = load_task_manifests(&args.tasks_dir)?;
    let selected = select_manifests(
        &manifests,
        &args.instance_ids,
        args.skip_tasks,
        args.max_tasks,
    )?;
    let selected_task_ids = selected
        .iter()
        .map(|manifest| manifest.instance_id.clone())
        .collect::<Vec<_>>();
    let patch_by_task = load_official_patches(
        &args.venv_python,
        &args.dataset_name,
        &args.dataset_split,
        selected
            .iter()
            .map(|manifest| manifest.instance_id.as_str())
            .collect::<Vec<_>>()
            .as_slice(),
    )?;
    let source_patch_config = SourcePatchConfig {
        repo_cache_dir: args.repo_cache_dir.clone(),
        work_root: args.source_work_root.clone(),
    };
    let split_entries = manifests
        .iter()
        .map(|manifest| CorpusEntry {
            task_id: manifest.instance_id.clone(),
            repo: manifest.repo.clone(),
        })
        .collect::<Vec<_>>();
    let split = split_corpus(
        &split_entries,
        SplitConfig {
            mode: SplitMode::InstanceAtomic,
            ..SplitConfig::default()
        },
    )?;
    let sha_by_task = manifests
        .iter()
        .map(|manifest| {
            (
                manifest.instance_id.clone(),
                manifest.official_test_patch_sha256.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_no_test_patch_leakage(&split.assignments, |task_id| {
        sha_by_task.get(task_id).map(String::as_str)
    })?;
    let bucket_by_task = split
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.task_id.clone(),
                assignment.bucket.slug().to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    fs::create_dir_all(&args.output)?;
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;
    let mut index = if args.resume_incomplete {
        let existing = read_index(&args.output)?;
        validate_resume_index(
            &existing,
            &args,
            &languages,
            &split.assignments,
            &store_path_rel,
            &selected_task_ids,
        )?;
        existing
    } else {
        CorpusIndex {
            schema_version: 1,
            corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
            embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
            dataset_name: args.dataset_name.clone(),
            dataset_split: args.dataset_split.clone(),
            complete: false,
            generated_at_unix_ms: unix_ms(),
            seed: args.seed,
            languages: languages.clone(),
            tasks_dir: args.tasks_dir.display().to_string(),
            store_path: store_path_rel.display().to_string(),
            corpus_sha256: None,
            selected_task_ids: selected_task_ids.clone(),
            entries: Vec::new(),
            split_assignments: split.assignments.clone(),
        }
    };
    if index.selected_task_ids.is_empty() {
        index.selected_task_ids = selected_task_ids.clone();
    }
    let mut completed_keys = validate_resume_rows(&args.output, &index, &store)?;
    write_index(&args.output, &index)?;

    // REQ-FLYWHEEL-PARALLEL — rayon-parallel patch generation.
    //
    // The original loop was strictly sequential at ~10 s/task (git cat-file +
    // tree-sitter AST parse + mutation operator + git diff). On the AMD
    // 9950X3D (16 cores / 30 threads, Zen 5, dual-CCD) this left 29 cores
    // idle while one core was saturated. Mutation testing in Rust is a
    // well-established rayon use case (Foundry's `--mutation-jobs` ships
    // exactly this pattern with 2.5× speedup at 4 workers).
    //
    // Concurrency model: each task is independent — distinct
    // source-work directory, distinct output path, read-only access to
    // the shared bare repo cache (git supports concurrent readers).
    // The OracleStore (RocksDB) is `Sync` via internal locks. The
    // `interrupted` flag is `Arc<AtomicBool>`. `completed_keys` is a
    // shared `HashSet<String>` borrowed immutably (read-only during the
    // per-category pass).
    //
    // Foundry recommends 16 MB stack per rayon thread for mutation
    // workloads; we match that. Defaults to `num_cpus::get()` (typically
    // 30 on this box) when --patch-workers=0.
    let patch_pool = {
        let want = if args.patch_workers == 0 {
            num_cpus::get()
        } else {
            args.patch_workers
        };
        rayon::ThreadPoolBuilder::new()
            .num_threads(want)
            .thread_name(|i| format!("mejepa-patch-{i}"))
            .stack_size(16 * 1024 * 1024)
            .build()
            .map_err(|err| format!("failed to build patch_workers pool: {err}"))?
    };

    for category in &categories {
        if interrupted.load(Ordering::SeqCst) {
            write_index(&args.output, &index)?;
            return Err(
                "generation interrupted by SIGINT before starting next corpus row; index remains complete=false"
                    .into(),
            );
        }
        // Phase 1: parallel patch generation across all selected manifests
        // for this category. Rayon workers stay on source-prep work only:
        // git/cache/worktree activity lives under the ext4 ephemeral roots.
        // Durable corpus patch files are persisted serially after oracle verdicts
        // return, so the archive root never sees 30 concurrent atomic
        // writes and failed oracle batches do not leave stale patch files.
        let category_value = *category;
        let interrupted_ref = &interrupted;
        let source_patch_config_ref = &source_patch_config;
        let patch_by_task_ref = &patch_by_task;
        let args_seed = args.seed;
        let completed_keys_ref = &completed_keys;
        type GeneratedPatch = (
            TaskManifest,
            context_graph_mejepa_corpus::patch_mutation::PatchMutationOutcome,
            String,
            MutationOutcome,
        );

        let results: Vec<Result<Option<(GeneratedPatch, SwebenchPrediction)>, String>> = patch_pool
            .install(|| {
                use rayon::prelude::*;
                selected
                    .par_iter()
                    .map(|manifest| -> Result<Option<(GeneratedPatch, SwebenchPrediction)>, String> {
                        let row_key = corpus_row_key(&manifest.instance_id, category_value);
                        if completed_keys_ref.contains(&row_key) {
                            return Ok(None);
                        }
                        if interrupted_ref.load(Ordering::SeqCst) {
                            return Ok(None);
                        }
                        let official_patch =
                            patch_by_task_ref.get(&manifest.instance_id).ok_or_else(|| {
                                format!(
                                    "official dataset patch missing after loader for {}",
                                    manifest.instance_id
                                )
                            })?;
                        let seed = stable_seed(args_seed, &manifest.instance_id, category_value);
                        let patch_outcome = mutate_task_source_patch(
                            &SourcePatchTask {
                                instance_id: manifest.instance_id.clone(),
                                repo: manifest.repo.clone(),
                                base_commit: manifest.base_commit.clone(),
                            },
                            category_value,
                            official_patch,
                            seed,
                            source_patch_config_ref,
                        )
                        .map_err(|err| {
                            format!(
                                "failed to build source-backed patch for task={} category={}: {} ({})",
                                manifest.instance_id,
                                category_value.slug(),
                                err,
                                err.code()
                            )
                        })?;
                        let patch_sha256 = sha256_text(&patch_outcome.mutated_patch);
                        let mutation = MutationOutcome {
                            category: category_value,
                            mutated_source: patch_outcome.mutated_patch.clone(),
                            seed,
                            mutation_site: None,
                        };
                        let prediction = SwebenchPrediction {
                            instance_id: manifest.instance_id.clone(),
                            model_name_or_path: format!("mejepa-{}", category_value.slug()),
                            model_patch: patch_outcome.mutated_patch.clone(),
                        };
                        Ok(Some((
                            (
                                manifest.clone(),
                                patch_outcome,
                                patch_sha256,
                                mutation,
                            ),
                            prediction,
                        )))
                    })
                    .collect()
            });

        // Drain results: short-circuit on first error so the CLI exits with
        // a structured failure rather than producing a half-baked corpus.
        let mut generated_patches: Vec<GeneratedPatch> = Vec::with_capacity(selected.len());
        let mut predictions: Vec<SwebenchPrediction> = Vec::with_capacity(selected.len());
        for result in results {
            match result {
                Ok(Some((row, prediction))) => {
                    generated_patches.push(row);
                    predictions.push(prediction);
                }
                Ok(None) => continue,
                Err(err) => {
                    write_index(&args.output, &index)?;
                    return Err(err.into());
                }
            }
        }
        if interrupted.load(Ordering::SeqCst) {
            write_index(&args.output, &index)?;
            return Err(
                "generation interrupted by SIGINT after parallel patch generation; index remains complete=false"
                    .into(),
            );
        }
        if predictions.is_empty() {
            continue;
        }
        let mut batch_config = SwebenchBatchOracleConfig::defaults_for(
            predictions,
            docker_run_id(&format!("{}-{}", args.run_id_prefix, category.slug()))?,
            args.output.join("swebench-runs"),
            args.venv_python.clone(),
        );
        batch_config.dataset_name = args.dataset_name.clone();
        batch_config.split = args.dataset_split.clone();
        batch_config.instance_timeout = Duration::from_secs(args.instance_timeout_secs);
        batch_config.overall_timeout = Duration::from_secs(args.overall_timeout_secs);
        batch_config.max_workers = args.oracle_workers;
        batch_config.interrupt_flag = Some(Arc::clone(&interrupted));
        let batch_result = run_swebench_lite_oracle_batch(&batch_config)?;

        // Phase 2: after the oracle returns, serialize durable writes for the
        // rows that have a verdict. This avoids creating patch files for an
        // oracle batch that never produced source-of-truth rows.
        for (manifest, patch_outcome, patch_sha256, mutation) in generated_patches {
            if interrupted.load(Ordering::SeqCst) {
                write_index(&args.output, &index)?;
                return Err(
                    "generation interrupted by SIGINT before durable row persistence; index remains complete=false"
                        .into(),
                );
            }
            let verdict = batch_result
                .verdicts
                .get(&manifest.instance_id)
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "batch oracle did not return verdict for {}|{}",
                        manifest.instance_id,
                        category.slug()
                    )
                })?;
            let (_patch_path, patch_path_rel) =
                write_mutated_patch(&args.output, &manifest.instance_id, &patch_outcome).map_err(
                    |err| {
                        format!(
                    "serial write_mutated_patch failed for task={} category={}: {} (debug={:?})",
                    manifest.instance_id,
                    category.slug(),
                    err,
                    err,
                )
                    },
                )?;
            store.put_corpus_row(&manifest.instance_id, *category, &mutation, &verdict)?;
            store.flush()?;
            let (stored_mutation, stored_verdict) = store
                .get_corpus_row(&manifest.instance_id, *category)?
                .ok_or("corpus row missing immediately after RocksDB write")?;
            if stored_mutation != mutation || stored_verdict != verdict {
                return Err(format!(
                    "corpus row readback mismatch for {}|{} immediately after RocksDB write",
                    manifest.instance_id,
                    category.slug()
                )
                .into());
            }
            let oracle_verdict_sha256 = sha256_json_value(&serde_json::to_value(&verdict)?);

            let entry = GeneratedEntry {
                task_id: manifest.instance_id.clone(),
                repo: manifest.repo.clone(),
                language: Language::Python,
                category: *category,
                bucket: bucket_by_task
                    .get(&manifest.instance_id)
                    .cloned()
                    .ok_or("selected task missing split bucket")?,
                patch_path: patch_path_rel.display().to_string(),
                mutation_note: patch_outcome.note.clone(),
                patch_sha256,
                oracle_verdict_sha256,
                oracle_all_passed: verdict.all_passed(),
                oracle_exception: verdict.exception.map(|class| class.slug().to_string()),
                oracle_per_test_count: verdict.per_test.len(),
            };
            index.entries.push(entry);
            completed_keys.insert(corpus_row_key(&manifest.instance_id, *category));
            write_index(&args.output, &index)?;
        }
    }
    let provenance = build_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or("provenance row missing immediately after RocksDB write")?;
    if stored_provenance != provenance {
        return Err("provenance readback mismatch immediately after RocksDB write".into());
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_stats_json(
        &args.output,
        &build_stats_json(&args.output, &index, &store)?,
    )?;
    write_index(&args.output, &index)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "status": "generated",
            "entries": index.entries.len(),
            "index": args.output.join("index.json"),
            "store": store_path,
        }))?
    );
    Ok(())
}

fn verify(mut args: VerifyArgs) -> CliResult<()> {
    args.corpus = require_corpus_path(&args.corpus, "verify.corpus")?;
    args.venv_python = require_runtime_python_path(&args.venv_python, "verify.venv_python")?;
    args.oracle_repeat_output = match args.oracle_repeat_output.take() {
        Some(path) => Some(require_durable_path(&path, "verify.oracle_repeat_output")?),
        None => None,
    };
    args.quarantine_config = match args.quarantine_config.take() {
        Some(path) => Some(require_durable_path(&path, "verify.quarantine_config")?),
        None => None,
    };
    validate_verify_args(&args)?;
    let quarantine = match &args.quarantine_config {
        Some(path) => load_corpus_quarantine(path)?,
        None => BTreeMap::new(),
    };
    let index = read_index(&args.corpus)?;
    validate_complete_index(&index)?;
    let store_path = resolve_corpus_path(&args.corpus, &index.store_path)?;
    let store = OracleStore::open(&store_path)?;
    let provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or_else(|| {
            format!(
                "missing provenance row for corpus_version={} embedder_version={}",
                index.corpus_version, index.embedder_version
            )
        })?;
    let expected_provenance = build_provenance(&index)?;
    let mut failures = Vec::new();
    if provenance != expected_provenance {
        failures.push(json!({
            "error": "provenance mismatch",
            "expected": expected_provenance.clone(),
            "actual": provenance.clone(),
        }));
    }
    let mut seen = BTreeSet::new();
    for entry in &index.entries {
        let key = format!("{}|{}", entry.task_id, entry.category.slug());
        if !seen.insert(key.clone()) {
            failures.push(json!({"key": key, "error": "duplicate index entry"}));
        }
        let patch_path = resolve_corpus_path(&args.corpus, &entry.patch_path)?;
        let patch_text = fs::read_to_string(&patch_path)?;
        let actual_hash = sha256_text(&patch_text);
        if actual_hash != entry.patch_sha256 {
            failures.push(json!({
                "key": key,
                "error": "patch hash mismatch",
                "expected": entry.patch_sha256,
                "actual": actual_hash,
            }));
        }
        match store.get_corpus_row(&entry.task_id, entry.category)? {
            Some((mutation, verdict)) => {
                if sha256_text(&mutation.mutated_source) != entry.patch_sha256 {
                    failures.push(json!({"key": key, "error": "RocksDB mutation hash mismatch"}));
                }
                let actual = sha256_json_value(&serde_json::to_value(&verdict)?);
                if actual != entry.oracle_verdict_sha256 {
                    failures.push(json!({
                        "key": key,
                        "error": "RocksDB verdict hash mismatch",
                        "expected": entry.oracle_verdict_sha256,
                        "actual": actual,
                    }));
                }
            }
            None => failures.push(json!({"key": key, "error": "missing RocksDB corpus row"})),
        }
    }
    let mutation_cf_count = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?;
    let provenance_cf_count = store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?;
    let iterated_corpus_rows = store.iter_corpus_rows()?;
    let expected_full_entries = index.split_assignments.len() * MutationCategory::all().len();
    if mutation_cf_count != index.entries.len() {
        failures.push(json!({
            "error": "mutation column-family count mismatch",
            "expected": index.entries.len(),
            "actual": mutation_cf_count,
        }));
    }
    if verdict_cf_count != index.entries.len() {
        failures.push(json!({
            "error": "verdict column-family count mismatch",
            "expected": index.entries.len(),
            "actual": verdict_cf_count,
        }));
    }
    if provenance_cf_count != 1 {
        failures.push(json!({
            "error": "provenance column-family count mismatch",
            "expected": 1,
            "actual": provenance_cf_count,
        }));
    }
    if iterated_corpus_rows.len() != index.entries.len() {
        failures.push(json!({
            "error": "iterated atomic corpus row count mismatch",
            "expected": index.entries.len(),
            "actual": iterated_corpus_rows.len(),
        }));
    }
    let mut oracle_sample = Vec::new();
    let mut oracle_repeat_tasks = Vec::new();
    let mut quarantine_covered_mismatches = Vec::new();
    let mut oracle_sample_count = 0usize;
    let oracle_sample_skipped_due_to_local_failures = !failures.is_empty();
    if !oracle_sample_skipped_due_to_local_failures {
        let sampled_entries = select_oracle_samples(
            &index.entries,
            args.sample_fraction,
            provenance.corpus_sha256.as_str(),
            &args.sample_keys,
        )?;
        oracle_sample_count = sampled_entries.len();
        for entry in &sampled_entries {
            let mut fresh_runs = Vec::with_capacity(args.oracle_repeat_runs);
            let mut repeat_runs_for_output = Vec::with_capacity(args.oracle_repeat_runs);
            let quarantine_entry = quarantine.get(&entry.task_id);
            for repeat_run_index in 1..=args.oracle_repeat_runs {
                let run_id = oracle_repeat_run_id(&args, entry, repeat_run_index)?;
                let fresh_verdict = run_oracle_for_entry(&args, &index, entry, &run_id)?;
                let fresh_run = oracle_repeat_run_json(run_id, &fresh_verdict)?;
                let fresh_hash = fresh_run
                    .get("oracle_verdict_sha256")
                    .and_then(Value::as_str)
                    .ok_or("oracle repeat run missing oracle_verdict_sha256")?
                    .to_string();
                let matches_persisted = fresh_hash == entry.oracle_verdict_sha256;
                if !matches_persisted {
                    let mismatch = json!({
                        "key": format!("{}|{}", entry.task_id, entry.category.slug()),
                        "run_index": repeat_run_index,
                        "error": "fresh oracle sample hash mismatch",
                        "expected": entry.oracle_verdict_sha256,
                        "actual": fresh_hash,
                    });
                    if let Some(quarantine_entry) = quarantine_entry {
                        quarantine_covered_mismatches.push(json!({
                            "key": format!("{}|{}", entry.task_id, entry.category.slug()),
                            "task_id": entry.task_id,
                            "category": entry.category.slug(),
                            "run_index": repeat_run_index,
                            "error": "quarantine covered fresh oracle sample hash mismatch",
                            "expected": entry.oracle_verdict_sha256,
                            "actual": fresh_hash,
                            "quarantine_reason": quarantine_entry.reason,
                            "quarantine_flakiness_rate": quarantine_entry.flakiness_rate,
                            "quarantine_oracle_runs": quarantine_entry.oracle_runs,
                        }));
                    } else {
                        failures.push(mismatch);
                    }
                }
                fresh_runs.push(json!({
                    "run_index": repeat_run_index,
                    "fresh_oracle_verdict_sha256": fresh_hash,
                    "fresh_all_passed": fresh_verdict.all_passed(),
                    "matches_persisted": matches_persisted,
                    "quarantine_covered_mismatch": !matches_persisted && quarantine_entry.is_some(),
                }));
                repeat_runs_for_output.push(fresh_run);
            }
            let first_run = fresh_runs
                .first()
                .ok_or("oracle_repeat_runs validation allowed zero repeat runs")?;
            oracle_repeat_tasks.push(build_oracle_repeat_task_observation(
                entry,
                repeat_runs_for_output,
            ));
            oracle_sample.push(json!({
                "task_id": entry.task_id,
                "category": entry.category.slug(),
                "persisted_oracle_verdict_sha256": entry.oracle_verdict_sha256,
                "fresh_oracle_verdict_sha256": first_run["fresh_oracle_verdict_sha256"].clone(),
                "persisted_all_passed": entry.oracle_all_passed,
                "fresh_all_passed": first_run["fresh_all_passed"].clone(),
                "quarantined_task": quarantine_entry.is_some(),
                "quarantine_reason": quarantine_entry.map(|entry| entry.reason.clone()),
                "matches_persisted": fresh_runs.iter().all(|run| run["matches_persisted"].as_bool() == Some(true)),
                "fresh_runs": fresh_runs,
            }));
        }
    }
    let oracle_repeat_output_path = args
        .oracle_repeat_output
        .as_ref()
        .map(|path| path.display().to_string());
    let mut oracle_repeat_output_written = false;
    if let Some(path) = &args.oracle_repeat_output {
        if !oracle_sample_skipped_due_to_local_failures {
            let observations = json!({ "tasks": oracle_repeat_tasks });
            write_json_checked(path, &observations)?;
            let readback: Value = serde_json::from_str(&fs::read_to_string(path)?)?;
            if readback != observations {
                return Err(format!(
                    "oracle repeat output readback mismatch at {}",
                    path.display()
                )
                .into());
            }
            oracle_repeat_output_written = true;
        }
    }
    let passes = failures.is_empty();
    let evidence = json!({
        "source_of_truth": {
            "index": args.corpus.join("index.json").display().to_string(),
            "store": store_path.display().to_string(),
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE_JSON,
        },
        "corpus_sha256": provenance.corpus_sha256,
        "entry_count": index.entries.len(),
        "task_manifest_count": index.split_assignments.len(),
        "expected_full_phase0_entries": expected_full_entries,
        "full_phase0_complete": index.entries.len() == expected_full_entries,
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
        "provenance_cf_count": provenance_cf_count,
        "sample_fraction": args.sample_fraction,
        "targeted_sample_keys": args.sample_keys,
        "oracle_sample_count": oracle_sample_count,
        "oracle_sample_skipped_due_to_local_failures": oracle_sample_skipped_due_to_local_failures,
        "oracle_repeat_runs_per_sample": args.oracle_repeat_runs,
        "oracle_repeat_output": oracle_repeat_output_path,
        "oracle_repeat_output_task_count": oracle_repeat_tasks.len(),
        "oracle_repeat_output_written": oracle_repeat_output_written,
        "quarantine_config": args.quarantine_config.as_ref().map(|path| path.display().to_string()),
        "quarantine_task_count": quarantine.len(),
        "quarantine_covered_mismatch_count": quarantine_covered_mismatches.len(),
        "quarantine_covered_mismatches": quarantine_covered_mismatches,
        "oracle_sample": oracle_sample,
        "failures": failures,
        "passes": passes,
    });
    write_json_checked(&args.corpus.join("verify-evidence.json"), &evidence)?;
    if !passes {
        return Err(format!(
            "corpus verification failed: {}",
            serde_json::to_string_pretty(&evidence)?
        )
        .into());
    }
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn stats(mut args: StatsArgs) -> CliResult<()> {
    args.corpus = require_corpus_path(&args.corpus, "stats.corpus")?;
    let index = read_index(&args.corpus)?;
    validate_complete_index(&index)?;
    let store_path = resolve_corpus_path(&args.corpus, &index.store_path)?;
    let store = OracleStore::open(&store_path)?;
    let stats = build_stats_json(&args.corpus, &index, &store)?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
}

fn prepare_images(args: PrepareImagesArgs) -> CliResult<()> {
    validate_prepare_images_args(&args)?;
    let evidence_path = require_durable_path(&args.evidence, "prepare_images.evidence")?;
    let manifests = load_task_manifests(&args.tasks_dir)?;
    let selected = select_manifests(
        &manifests,
        &args.instance_ids,
        args.skip_tasks,
        args.max_tasks,
    )?;
    let selected_task_ids = selected
        .iter()
        .map(|manifest| manifest.instance_id.clone())
        .collect::<Vec<_>>();
    let target_images = selected_task_ids
        .iter()
        .map(|task_id| swebench_instance_image_ref(&args.namespace, task_id))
        .collect::<CliResult<Vec<_>>>()?;
    let docker_df_before = docker_stdout(&["system", "df"])?;
    let local_images_before = docker_stdout(&["images", "--format", "{{.Repository}}:{{.Tag}}"])?;
    let local_images_before = local_images_before
        .lines()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let missing = target_images
        .iter()
        .filter(|image| !local_images_before.contains(*image))
        .cloned()
        .collect::<Vec<_>>();
    let started = Instant::now();
    let pull_results = pull_images_bounded(
        missing.clone(),
        args.parallel,
        Duration::from_secs(args.pull_timeout_secs),
    )?;
    let docker_df_after = docker_stdout(&["system", "df"])?;
    let local_images_after = docker_stdout(&["images", "--format", "{{.Repository}}:{{.Tag}}"])?;
    let local_images_after = local_images_after
        .lines()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let missing_after = target_images
        .iter()
        .filter(|image| !local_images_after.contains(*image))
        .cloned()
        .collect::<Vec<_>>();
    let failures = pull_results
        .iter()
        .filter(|result| result.get("success").and_then(Value::as_bool) != Some(true))
        .cloned()
        .collect::<Vec<_>>();
    let passes = missing_after.is_empty() && failures.is_empty();
    let evidence = json!({
        "source_of_truth": {
            "tasks_dir": args.tasks_dir.display().to_string(),
            "docker": "docker images --format {{.Repository}}:{{.Tag}}",
        },
        "namespace": args.namespace,
        "parallel": args.parallel,
        "pull_timeout_secs": args.pull_timeout_secs,
        "selected_task_count": selected_task_ids.len(),
        "selected_task_ids": selected_task_ids,
        "target_image_count": target_images.len(),
        "target_images": target_images,
        "missing_before_count": missing.len(),
        "missing_before": missing,
        "pull_results": pull_results,
        "missing_after_count": missing_after.len(),
        "missing_after": missing_after,
        "duration_secs": duration_secs(started.elapsed()),
        "docker_system_df_before": docker_df_before,
        "docker_system_df_after": docker_df_after,
        "passes": passes,
    });
    write_json_checked(&evidence_path, &evidence)?;
    if !passes {
        return Err(format!(
            "SWE-bench image preparation failed; evidence written to {}",
            evidence_path.display()
        )
        .into());
    }
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn cleanup(mut args: CleanupArgs) -> CliResult<()> {
    args.corpus = require_corpus_path(&args.corpus, "cleanup.corpus")?;
    validate_cleanup_args(&args)?;
    let index = read_index(&args.corpus)?;
    validate_complete_index(&index)?;
    let task_ids = index
        .entries
        .iter()
        .map(|entry| entry.task_id.clone())
        .collect::<BTreeSet<_>>();
    if task_ids.is_empty() {
        return Err("cleanup target corpus has no task ids".into());
    }
    let target_images = task_ids
        .iter()
        .map(|task_id| swebench_instance_image_ref(&args.namespace, task_id))
        .collect::<CliResult<BTreeSet<_>>>()?;
    let docker_df_before = docker_stdout(&["system", "df"])?;
    let local_images_before = docker_stdout(&["images", "--format", "{{.Repository}}:{{.Tag}}"])?;
    let local_images_before = local_images_before
        .lines()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let present_before = target_images
        .intersection(&local_images_before)
        .cloned()
        .collect::<Vec<_>>();
    let mut removed_containers = Vec::<Value>::new();
    let mut active_containers = Vec::<Value>::new();
    if args.remove_containers {
        for row in docker_stdout(&[
            "ps",
            "-a",
            "--format",
            "{{.ID}}\t{{.Image}}\t{{.Status}}\t{{.Names}}",
        ])?
        .lines()
        {
            let parts = row.splitn(4, '\t').collect::<Vec<_>>();
            if parts.len() != 4 || !target_images.contains(parts[1]) {
                continue;
            }
            let container = json!({
                "id": parts[0],
                "image": parts[1],
                "status": parts[2],
                "name": parts[3],
            });
            if parts[2] == "Up" || parts[2].starts_with("Up ") {
                active_containers.push(container);
            } else if args.execute {
                docker_status(&["rm", parts[0]])?;
                removed_containers.push(container);
            } else {
                removed_containers.push(container);
            }
        }
    }
    if !active_containers.is_empty() {
        return Err(format!(
            "refusing cleanup while SWE-bench target containers are running: {}",
            serde_json::to_string_pretty(&active_containers)?
        )
        .into());
    }

    let mut removed_images = Vec::<String>::new();
    if args.execute {
        for image in &present_before {
            docker_status(&["rmi", image.as_str()])?;
            removed_images.push(image.clone());
        }
    }

    let docker_df_after = docker_stdout(&["system", "df"])?;
    let local_images_after = docker_stdout(&["images", "--format", "{{.Repository}}:{{.Tag}}"])?;
    let local_images_after = local_images_after
        .lines()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let present_after = target_images
        .intersection(&local_images_after)
        .cloned()
        .collect::<Vec<_>>();
    if args.execute && !present_after.is_empty() {
        return Err(format!(
            "cleanup did not remove all target images; remaining={present_after:?}"
        )
        .into());
    }
    let cleanup_passes = !args.execute || present_after.is_empty();

    let evidence = json!({
        "source_of_truth": {
            "index": args.corpus.join("index.json").display().to_string(),
            "docker": "docker images --format {{.Repository}}:{{.Tag}}",
        },
        "mode": if args.execute { "executed" } else { "dry_run" },
        "namespace": args.namespace,
        "task_count": task_ids.len(),
        "target_image_count": target_images.len(),
        "target_images_present_before": present_before,
        "target_images_present_after": present_after,
        "removed_images": removed_images,
        "remove_containers": args.remove_containers,
        "target_containers_removed_or_would_remove": removed_containers,
        "docker_system_df_before": docker_df_before,
        "docker_system_df_after": docker_df_after,
        "passes": cleanup_passes,
    });
    write_json_checked(&args.corpus.join("cleanup-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn merge(mut args: MergeArgs) -> CliResult<()> {
    args.output = require_corpus_path(&args.output, "merge.output")?;
    args.shards = args
        .shards
        .iter()
        .map(|shard| require_corpus_path(shard, "merge.shard"))
        .collect::<CliResult<Vec<_>>>()?;
    if args.shards.is_empty() {
        return Err("merge requires at least one --shard".into());
    }
    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;

    let mut merged_index: Option<CorpusIndex> = None;
    let mut entries = Vec::<GeneratedEntry>::new();
    let mut seen = BTreeSet::<String>::new();
    for shard in &args.shards {
        let shard_index = read_index(shard)?;
        validate_complete_index(&shard_index)?;
        let shard_store_path = resolve_corpus_path(shard, &shard_index.store_path)?;
        let shard_store = OracleStore::open(&shard_store_path)?;
        if let Some(existing) = &merged_index {
            validate_merge_compatible(existing, &shard_index, shard)?;
        } else {
            let index = CorpusIndex {
                schema_version: shard_index.schema_version,
                corpus_version: shard_index.corpus_version.clone(),
                embedder_version: shard_index.embedder_version.clone(),
                dataset_name: shard_index.dataset_name.clone(),
                dataset_split: shard_index.dataset_split.clone(),
                complete: false,
                generated_at_unix_ms: unix_ms(),
                seed: shard_index.seed,
                languages: shard_index.languages.clone(),
                tasks_dir: shard_index.tasks_dir.clone(),
                store_path: store_path_rel.display().to_string(),
                corpus_sha256: None,
                selected_task_ids: Vec::new(),
                entries: Vec::new(),
                split_assignments: shard_index.split_assignments.clone(),
            };
            write_index(&args.output, &index)?;
            merged_index = Some(index);
        }

        for entry in &shard_index.entries {
            let key = format!("{}|{}", entry.task_id, entry.category.slug());
            if !seen.insert(key.clone()) {
                return Err(format!("duplicate merged corpus row key: {key}").into());
            }
            let source_patch_path = resolve_corpus_path(shard, &entry.patch_path)?;
            let patch_text = fs::read_to_string(&source_patch_path)?;
            let patch_hash = sha256_text(&patch_text);
            if patch_hash != entry.patch_sha256 {
                return Err(format!(
                    "shard patch hash mismatch for {key}: expected {} actual {}",
                    entry.patch_sha256, patch_hash
                )
                .into());
            }
            let (mutation, verdict) = shard_store
                .get_corpus_row(&entry.task_id, entry.category)?
                .ok_or_else(|| format!("shard RocksDB row missing for {key}"))?;
            if sha256_text(&mutation.mutated_source) != entry.patch_sha256 {
                return Err(format!("shard mutation hash mismatch for {key}").into());
            }
            let verdict_hash = sha256_json_value(&serde_json::to_value(&verdict)?);
            if verdict_hash != entry.oracle_verdict_sha256 {
                return Err(format!(
                    "shard verdict hash mismatch for {key}: expected {} actual {}",
                    entry.oracle_verdict_sha256, verdict_hash
                )
                .into());
            }
            let destination_patch_path = resolve_corpus_path(&args.output, &entry.patch_path)?;
            write_text_checked(&destination_patch_path, &patch_text)?;
            store.put_corpus_row(&entry.task_id, entry.category, &mutation, &verdict)?;
            store.flush()?;
            let (stored_mutation, stored_verdict) = store
                .get_corpus_row(&entry.task_id, entry.category)?
                .ok_or_else(|| {
                    format!("merged RocksDB row missing immediately after write for {key}")
                })?;
            if stored_mutation != mutation || stored_verdict != verdict {
                return Err(format!("merged RocksDB row readback mismatch for {key}").into());
            }
            entries.push(entry.clone());
            if let Some(index) = &mut merged_index {
                index.entries = entries.clone();
                write_index(&args.output, index)?;
            }
        }
    }

    let mut index = merged_index.ok_or("merge produced no index")?;
    let task_order = index
        .split_assignments
        .iter()
        .enumerate()
        .map(|(position, assignment)| (assignment.task_id.clone(), position))
        .collect::<BTreeMap<_, _>>();
    entries.sort_by_key(|entry| {
        (
            task_order
                .get(&entry.task_id)
                .copied()
                .unwrap_or(usize::MAX),
            category_position(entry.category),
        )
    });
    index.selected_task_ids = entries
        .iter()
        .map(|entry| entry.task_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    index.entries = entries;
    let expected_entries = index.split_assignments.len() * MutationCategory::all().len();
    if !args.allow_partial && index.entries.len() != expected_entries {
        write_index(&args.output, &index)?;
        return Err(format!(
            "merged corpus is partial: entries={} expected_full_phase0_entries={expected_entries}; pass --allow-partial only for smoke/debug merges",
            index.entries.len()
        )
        .into());
    }
    let provenance = build_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or("merged provenance row missing immediately after RocksDB write")?;
    if stored_provenance != provenance {
        return Err("merged provenance readback mismatch immediately after RocksDB write".into());
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_stats_json(
        &args.output,
        &build_stats_json(&args.output, &index, &store)?,
    )?;
    write_index(&args.output, &index)?;
    let evidence = json!({
        "status": "merged",
        "entries": index.entries.len(),
        "expected_full_phase0_entries": expected_entries,
        "full_phase0_complete": index.entries.len() == expected_entries,
        "index": args.output.join("index.json"),
        "store": store_path,
        "shards": args.shards,
        "corpus_sha256": provenance.corpus_sha256,
    });
    write_json_checked(&args.output.join("merge-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn normalize(mut args: NormalizeArgs) -> CliResult<()> {
    args.corpus = require_corpus_path(&args.corpus, "normalize.corpus")?;
    args.output = require_corpus_path(&args.output, "normalize.output")?;
    validate_normalize_args(&args)?;
    let source_index = read_index(&args.corpus)?;
    validate_complete_index(&source_index)?;
    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;

    let source_store_path = resolve_corpus_path(&args.corpus, &source_index.store_path)?;
    let source_store = OracleStore::open(&source_store_path)?;
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;

    let mut index = CorpusIndex {
        schema_version: source_index.schema_version,
        corpus_version: source_index.corpus_version.clone(),
        embedder_version: source_index.embedder_version.clone(),
        dataset_name: source_index.dataset_name.clone(),
        dataset_split: source_index.dataset_split.clone(),
        complete: false,
        generated_at_unix_ms: unix_ms(),
        seed: source_index.seed,
        languages: source_index.languages.clone(),
        tasks_dir: source_index.tasks_dir.clone(),
        store_path: store_path_rel.display().to_string(),
        corpus_sha256: None,
        selected_task_ids: source_index.selected_task_ids.clone(),
        entries: Vec::with_capacity(source_index.entries.len()),
        split_assignments: source_index.split_assignments.clone(),
    };
    write_index(&args.output, &index)?;

    let mut seen = BTreeSet::<String>::new();
    let mut changed_entries = 0usize;
    let mut changed_examples = Vec::<Value>::new();
    for entry in &source_index.entries {
        let key = format!("{}|{}", entry.task_id, entry.category.slug());
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate source corpus row key: {key}").into());
        }
        let source_patch_path = resolve_corpus_path(&args.corpus, &entry.patch_path)?;
        let patch_text = fs::read_to_string(&source_patch_path)?;
        let patch_hash = sha256_text(&patch_text);
        if patch_hash != entry.patch_sha256 {
            return Err(format!(
                "source patch hash mismatch for {key}: expected {} actual {}",
                entry.patch_sha256, patch_hash
            )
            .into());
        }
        let (mutation, verdict) = source_store
            .get_corpus_row(&entry.task_id, entry.category)?
            .ok_or_else(|| format!("source RocksDB row missing for {key}"))?;
        if sha256_text(&mutation.mutated_source) != entry.patch_sha256 {
            return Err(format!("source mutation hash mismatch for {key}").into());
        }

        let normalized_verdict = normalize_swebench_oracle_verdict(verdict.clone());
        let normalized_hash = sha256_json_value(&serde_json::to_value(&normalized_verdict)?);
        let normalized_exception = normalized_verdict
            .exception
            .map(|class| class.slug().to_string());
        let row_changed = normalized_verdict != verdict
            || normalized_hash != entry.oracle_verdict_sha256
            || normalized_verdict.all_passed() != entry.oracle_all_passed
            || normalized_exception != entry.oracle_exception
            || normalized_verdict.per_test.len() != entry.oracle_per_test_count;
        if row_changed {
            changed_entries += 1;
            if changed_examples.len() < 20 {
                changed_examples.push(json!({
                    "key": key.clone(),
                    "old_oracle_verdict_sha256": entry.oracle_verdict_sha256.clone(),
                    "new_oracle_verdict_sha256": normalized_hash.clone(),
                    "old_exception": entry.oracle_exception.clone(),
                    "new_exception": normalized_exception.clone(),
                }));
            }
        }

        let destination_patch_path = resolve_corpus_path(&args.output, &entry.patch_path)?;
        write_text_checked(&destination_patch_path, &patch_text)?;
        store.put_corpus_row(
            &entry.task_id,
            entry.category,
            &mutation,
            &normalized_verdict,
        )?;
        store.flush()?;
        let (stored_mutation, stored_verdict) = store
            .get_corpus_row(&entry.task_id, entry.category)?
            .ok_or_else(|| {
                format!("normalized RocksDB row missing immediately after write for {key}")
            })?;
        if stored_mutation != mutation || stored_verdict != normalized_verdict {
            return Err(format!("normalized RocksDB row readback mismatch for {key}").into());
        }

        let mut normalized_entry = entry.clone();
        normalized_entry.oracle_verdict_sha256 = normalized_hash;
        normalized_entry.oracle_all_passed = normalized_verdict.all_passed();
        normalized_entry.oracle_exception = normalized_exception;
        normalized_entry.oracle_per_test_count = normalized_verdict.per_test.len();
        index.entries.push(normalized_entry);
        write_index(&args.output, &index)?;
    }

    let mutation_cf_count = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?;
    let iterated_corpus_rows = store.iter_corpus_rows()?;
    if mutation_cf_count != index.entries.len()
        || verdict_cf_count != index.entries.len()
        || iterated_corpus_rows.len() != index.entries.len()
    {
        return Err(format!(
            "normalized corpus store count mismatch: index={} mutation_cf={} verdict_cf={} iterated_rows={}",
            index.entries.len(),
            mutation_cf_count,
            verdict_cf_count,
            iterated_corpus_rows.len()
        )
        .into());
    }

    let provenance = build_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or("normalized provenance row missing immediately after RocksDB write")?;
    if stored_provenance != provenance {
        return Err(
            "normalized provenance readback mismatch immediately after RocksDB write".into(),
        );
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_stats_json(
        &args.output,
        &build_stats_json(&args.output, &index, &store)?,
    )?;
    write_index(&args.output, &index)?;

    let evidence = json!({
        "status": "normalized",
        "source_of_truth": {
            "input_index": args.corpus.join("index.json").display().to_string(),
            "input_store": source_store_path.display().to_string(),
            "output_index": args.output.join("index.json").display().to_string(),
            "output_store": store_path.display().to_string(),
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE_JSON,
        },
        "source_corpus_sha256": source_index.corpus_sha256,
        "normalized_corpus_sha256": provenance.corpus_sha256,
        "entries": index.entries.len(),
        "changed_entries": changed_entries,
        "changed_examples": changed_examples,
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?,
        "passes": true,
    });
    write_json_checked(&args.output.join("normalize-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn prune_quarantine(mut args: PruneQuarantineArgs) -> CliResult<()> {
    args.corpus = require_corpus_path(&args.corpus, "prune_quarantine.corpus")?;
    args.output = require_corpus_path(&args.output, "prune_quarantine.output")?;
    args.quarantine_config = require_durable_path(
        &args.quarantine_config,
        "prune_quarantine.quarantine_config",
    )?;
    validate_prune_quarantine_args(&args)?;
    prune_quarantine_impl(args)
}

fn prune_quarantine_impl(args: PruneQuarantineArgs) -> CliResult<()> {
    let quarantine = load_corpus_quarantine(&args.quarantine_config)?;
    if quarantine.is_empty() {
        return Err("prune-quarantine requires at least one quarantined task".into());
    }
    let quarantined_tasks = quarantine.keys().cloned().collect::<BTreeSet<_>>();
    let source_index = read_index(&args.corpus)?;
    validate_complete_index(&source_index)?;
    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;

    let source_store_path = resolve_corpus_path(&args.corpus, &source_index.store_path)?;
    let source_store = OracleStore::open(&source_store_path)?;
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;

    let mut index = CorpusIndex {
        schema_version: source_index.schema_version,
        corpus_version: source_index.corpus_version.clone(),
        embedder_version: source_index.embedder_version.clone(),
        dataset_name: source_index.dataset_name.clone(),
        dataset_split: source_index.dataset_split.clone(),
        complete: false,
        generated_at_unix_ms: unix_ms(),
        seed: source_index.seed,
        languages: source_index.languages.clone(),
        tasks_dir: source_index.tasks_dir.clone(),
        store_path: store_path_rel.display().to_string(),
        corpus_sha256: None,
        selected_task_ids: Vec::new(),
        entries: Vec::new(),
        // Preserve the original split assignment source of truth. This artifact
        // is a quarantined-row projection of the corpus, not a new split.
        split_assignments: source_index.split_assignments.clone(),
    };
    write_index(&args.output, &index)?;

    let mut seen = BTreeSet::<String>::new();
    let mut excluded_entries = Vec::<Value>::new();
    for entry in &source_index.entries {
        let key = corpus_row_key(&entry.task_id, entry.category);
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate source corpus row key: {key}").into());
        }
        let source_patch_path = resolve_corpus_path(&args.corpus, &entry.patch_path)?;
        let patch_text = fs::read_to_string(&source_patch_path)?;
        let patch_hash = sha256_text(&patch_text);
        if patch_hash != entry.patch_sha256 {
            return Err(format!(
                "source patch hash mismatch for {key}: expected {} actual {}",
                entry.patch_sha256, patch_hash
            )
            .into());
        }
        let (mutation, verdict) = source_store
            .get_corpus_row(&entry.task_id, entry.category)?
            .ok_or_else(|| format!("source RocksDB row missing for {key}"))?;
        if sha256_text(&mutation.mutated_source) != entry.patch_sha256 {
            return Err(format!("source mutation hash mismatch for {key}").into());
        }
        if quarantined_tasks.contains(&entry.task_id) {
            excluded_entries.push(json!({
                "task_id": entry.task_id,
                "category": entry.category.slug(),
                "reason": quarantine
                    .get(&entry.task_id)
                    .map(|entry| entry.reason.clone())
                    .unwrap_or_default(),
            }));
            continue;
        }

        let destination_patch_path = resolve_corpus_path(&args.output, &entry.patch_path)?;
        write_text_checked(&destination_patch_path, &patch_text)?;
        store.put_corpus_row(&entry.task_id, entry.category, &mutation, &verdict)?;
        store.flush()?;
        let (stored_mutation, stored_verdict) = store
            .get_corpus_row(&entry.task_id, entry.category)?
            .ok_or_else(|| {
                format!("pruned RocksDB row missing immediately after write for {key}")
            })?;
        if stored_mutation != mutation || stored_verdict != verdict {
            return Err(format!("pruned RocksDB row readback mismatch for {key}").into());
        }
        index.entries.push(entry.clone());
        write_index(&args.output, &index)?;
    }

    index.selected_task_ids = index
        .entries
        .iter()
        .map(|entry| entry.task_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    let mutation_cf_count = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?;
    let iterated_corpus_rows = store.iter_corpus_rows()?;
    if mutation_cf_count != index.entries.len()
        || verdict_cf_count != index.entries.len()
        || iterated_corpus_rows.len() != index.entries.len()
    {
        return Err(format!(
            "pruned corpus store count mismatch: index={} mutation_cf={} verdict_cf={} iterated_rows={}",
            index.entries.len(),
            mutation_cf_count,
            verdict_cf_count,
            iterated_corpus_rows.len()
        )
        .into());
    }

    let provenance = build_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or("pruned provenance row missing immediately after RocksDB write")?;
    if stored_provenance != provenance {
        return Err("pruned provenance readback mismatch immediately after RocksDB write".into());
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_stats_json(
        &args.output,
        &build_stats_json(&args.output, &index, &store)?,
    )?;
    write_index(&args.output, &index)?;

    let evidence = json!({
        "status": "pruned_quarantine",
        "source_of_truth": {
            "input_index": args.corpus.join("index.json").display().to_string(),
            "input_store": source_store_path.display().to_string(),
            "quarantine_config": args.quarantine_config.display().to_string(),
            "output_index": args.output.join("index.json").display().to_string(),
            "output_store": store_path.display().to_string(),
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE_JSON,
        },
        "source_corpus_sha256": source_index.corpus_sha256,
        "pruned_corpus_sha256": provenance.corpus_sha256,
        "quarantine_config_sha256": sha256_path(&args.quarantine_config)?,
        "source_entries": source_index.entries.len(),
        "entries": index.entries.len(),
        "excluded_entry_count": excluded_entries.len(),
        "excluded_task_count": quarantined_tasks.len(),
        "excluded_task_ids": quarantined_tasks,
        "excluded_entries": excluded_entries,
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?,
        "passes": true,
    });
    write_json_checked(
        &args.output.join("prune-quarantine-evidence.json"),
        &evidence,
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn generate_fixtures(mut args: GenerateFixturesArgs) -> CliResult<()> {
    args.output = require_corpus_path(&args.output, "generate_fixtures.output")?;
    if args.tasks_per_language == 0 {
        return Err("tasks_per_language must be > 0".into());
    }
    if args.tool_timeout_secs == 0 {
        return Err("tool_timeout_secs must be > 0".into());
    }
    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;

    let fixtures = build_fixture_tasks(args.tasks_per_language, args.seed)
        .map_err(|err| format!("{err} ({})", err.code()))?;
    let split_entries = fixtures
        .iter()
        .map(|fixture| CorpusEntry {
            task_id: fixture.task_id.clone(),
            repo: fixture.repo.clone(),
        })
        .collect::<Vec<_>>();
    let split = split_corpus(
        &split_entries,
        SplitConfig {
            mode: SplitMode::InstanceAtomic,
            ..SplitConfig::default()
        },
    )?;
    let source_sha_by_task = fixtures
        .iter()
        .map(|fixture| (fixture.task_id.clone(), fixture.source_sha256.clone()))
        .collect::<BTreeMap<_, _>>();
    assert_no_test_patch_leakage(&split.assignments, |task_id| {
        source_sha_by_task.get(task_id).map(String::as_str)
    })?;
    let bucket_by_task = split
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.task_id.clone(),
                assignment.bucket.slug().to_string(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;
    let languages = non_python_languages().to_vec();
    let mut index = CorpusIndex {
        schema_version: 1,
        corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
        embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
        dataset_name: "contextgraph/generated-multilang-fixtures".to_string(),
        dataset_split: "generated".to_string(),
        complete: false,
        generated_at_unix_ms: unix_ms(),
        seed: args.seed,
        languages,
        tasks_dir: "generated-multilang-fixtures".to_string(),
        store_path: store_path_rel.display().to_string(),
        corpus_sha256: None,
        selected_task_ids: fixtures
            .iter()
            .map(|fixture| fixture.task_id.clone())
            .collect(),
        entries: Vec::with_capacity(fixtures.len() * MutationCategory::all().len()),
        split_assignments: split.assignments.clone(),
    };
    write_index(&args.output, &index)?;

    let fixture_manifest_path = args.output.join("fixture-manifest.json");
    write_json_checked(&fixture_manifest_path, &serde_json::to_value(&fixtures)?)?;
    let mut toolchain_rows = Vec::new();
    for fixture in &fixtures {
        for category in MutationCategory::all() {
            let seed = stable_seed(args.seed, &fixture.task_id, category);
            let config = MutationConfig {
                seed,
                alternate_source: (category == MutationCategory::WrongFile)
                    .then(|| fixture.alternate_source.clone()),
            };
            let mutation =
                apply_mutation_for_language(fixture.language, category, &fixture.source, config)
                    .map_err(|err| {
                        format!(
                            "fixture mutation failed for {}|{} language={}: {} ({})",
                            fixture.task_id,
                            category.slug(),
                            fixture.language.slug(),
                            err,
                            err.code()
                        )
                    })?;
            let relative_path = fixture_mutation_relative_path(fixture, category);
            let source_path = args.output.join(&relative_path);
            write_text_checked(&source_path, &mutation.mutated_source)?;
            let expected_success = category != MutationCategory::CompileError;
            let report = validate_toolchain(
                fixture.language,
                &source_path,
                &args
                    .output
                    .join("toolchain-work")
                    .join(&fixture.task_id)
                    .join(category.slug()),
                expected_success,
                Duration::from_secs(args.tool_timeout_secs),
            )?;
            let report_path = args
                .output
                .join("toolchain-reports")
                .join(&fixture.task_id)
                .join(format!("{}.json", category.slug()));
            write_json_checked(&report_path, &serde_json::to_value(&report)?)?;
            let readback: Value = serde_json::from_str(&fs::read_to_string(&report_path)?)?;
            if readback != serde_json::to_value(&report)? {
                return Err(format!(
                    "toolchain report readback mismatch for {}|{}",
                    fixture.task_id,
                    category.slug()
                )
                .into());
            }

            let verdict = toolchain_verdict(&fixture.task_id, category, expected_success);
            store.put_corpus_row(&fixture.task_id, category, &mutation, &verdict)?;
            store.flush()?;
            let (stored_mutation, stored_verdict) = store
                .get_corpus_row(&fixture.task_id, category)?
                .ok_or("fixture corpus row missing immediately after RocksDB write")?;
            if stored_mutation != mutation || stored_verdict != verdict {
                return Err(format!(
                    "fixture RocksDB row readback mismatch for {}|{}",
                    fixture.task_id,
                    category.slug()
                )
                .into());
            }

            let oracle_verdict_sha256 = sha256_json_value(&serde_json::to_value(&verdict)?);
            index.entries.push(GeneratedEntry {
                task_id: fixture.task_id.clone(),
                repo: fixture.repo.clone(),
                language: fixture.language,
                category,
                bucket: bucket_by_task
                    .get(&fixture.task_id)
                    .cloned()
                    .ok_or("fixture task missing split bucket")?,
                patch_path: relative_path.display().to_string(),
                mutation_note: mutation
                    .mutation_site
                    .as_ref()
                    .map(|site| site.note.clone())
                    .unwrap_or_else(|| format!("{} fixture mutation", category.slug())),
                patch_sha256: fixture_sha256_text(&mutation.mutated_source),
                oracle_verdict_sha256,
                oracle_all_passed: verdict.all_passed(),
                oracle_exception: verdict.exception.map(|class| class.slug().to_string()),
                oracle_per_test_count: verdict.per_test.len(),
            });
            toolchain_rows.push(json!({
                "task_id": fixture.task_id.clone(),
                "language": fixture.language.slug(),
                "category": category.slug(),
                "source": source_path.display().to_string(),
                "report": report_path.display().to_string(),
                "expected_success": expected_success,
                "status_success": report.status_success,
            }));
            write_index(&args.output, &index)?;
        }
    }

    let expected_entries = fixtures.len() * MutationCategory::all().len();
    let mutation_cf_count = store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?;
    if index.entries.len() != expected_entries
        || mutation_cf_count != expected_entries
        || verdict_cf_count != expected_entries
    {
        return Err(format!(
            "fixture corpus count mismatch: index={} expected={} mutation_cf={} verdict_cf={}",
            index.entries.len(),
            expected_entries,
            mutation_cf_count,
            verdict_cf_count
        )
        .into());
    }
    let provenance = build_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or("fixture provenance row missing immediately after RocksDB write")?;
    if stored_provenance != provenance {
        return Err("fixture provenance readback mismatch immediately after RocksDB write".into());
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_stats_json(
        &args.output,
        &build_stats_json(&args.output, &index, &store)?,
    )?;
    write_index(&args.output, &index)?;
    let evidence = json!({
        "status": "generated_fixtures",
        "source_of_truth": {
            "index": args.output.join("index.json").display().to_string(),
            "store": store_path.display().to_string(),
            "fixture_manifest": fixture_manifest_path.display().to_string(),
            "toolchain_reports": args.output.join("toolchain-reports").display().to_string(),
            "stats": args.output.join("stats.json").display().to_string(),
        },
        "tasks_per_language": args.tasks_per_language,
        "language_count": non_python_languages().len(),
        "fixture_task_count": fixtures.len(),
        "entry_count": index.entries.len(),
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?,
        "toolchain_report_count": toolchain_rows.len(),
        "corpus_sha256": provenance.corpus_sha256,
        "passes": true,
    });
    write_json_checked(&args.output.join("verify-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn fixture_mutation_relative_path(fixture: &FixtureTask, category: MutationCategory) -> PathBuf {
    PathBuf::from("mutations")
        .join(&fixture.task_id)
        .join(format!("{}.{}", category.slug(), fixture.extension))
}

fn toolchain_verdict(
    task_id: &str,
    category: MutationCategory,
    expected_success: bool,
) -> OracleVerdict {
    let outcome = if expected_success {
        TestOutcome::Pass
    } else {
        TestOutcome::Error
    };
    OracleVerdict {
        per_test: vec![PerTestOutcome {
            test_id: format!("toolchain::{task_id}::{}", category.slug()),
            outcome,
            runtime_ms: -1,
        }],
        exception: (!expected_success).then_some(ExceptionClass::Other),
        evidence_unavailable: false,
    }
}

fn phase_c_verify(mut args: PhaseCVerifyArgs) -> CliResult<()> {
    args.python_corpus = require_corpus_path(&args.python_corpus, "phase_c.python_corpus")?;
    args.non_python_corpus =
        require_corpus_path(&args.non_python_corpus, "phase_c.non_python_corpus")?;
    args.evidence = require_durable_path(&args.evidence, "phase_c.evidence")?;
    if args.min_entries_per_non_python_language == 0 {
        return Err("min_entries_per_non_python_language must be > 0".into());
    }

    let python_index = read_index(&args.python_corpus)?;
    validate_complete_index(&python_index)?;
    let non_python_index = read_index(&args.non_python_corpus)?;
    validate_complete_index(&non_python_index)?;
    let python_store_path = resolve_corpus_path(&args.python_corpus, &python_index.store_path)?;
    let non_python_store_path =
        resolve_corpus_path(&args.non_python_corpus, &non_python_index.store_path)?;
    let python_store = OracleStore::open(&python_store_path)?;
    let non_python_store = OracleStore::open(&non_python_store_path)?;
    let python_stats = build_stats_json(&args.python_corpus, &python_index, &python_store)?;
    let non_python_stats = build_stats_json(
        &args.non_python_corpus,
        &non_python_index,
        &non_python_store,
    )?;
    let python_verify = read_json_required(&args.python_corpus.join("verify-evidence.json"))?;
    let non_python_verify =
        read_json_required(&args.non_python_corpus.join("verify-evidence.json"))?;

    let mut failures = Vec::<Value>::new();
    if python_verify.get("passes").and_then(Value::as_bool) != Some(true) {
        failures.push(json!({"corpus": "python", "error": "verify-evidence passes was not true"}));
    }
    if non_python_verify.get("passes").and_then(Value::as_bool) != Some(true) {
        failures
            .push(json!({"corpus": "non_python", "error": "verify-evidence passes was not true"}));
    }
    if python_stats
        .get("full_ship_gate_300x8")
        .and_then(|value| value.get("passes"))
        .and_then(Value::as_bool)
        != Some(true)
    {
        failures.push(json!({"corpus": "python", "error": "300x8 Python ship gate failed"}));
    }
    let expected_non_python = non_python_languages()
        .iter()
        .map(|language| language.slug().to_string())
        .collect::<BTreeSet<_>>();
    let actual_non_python = non_python_index
        .languages
        .iter()
        .map(|language| language.slug().to_string())
        .collect::<BTreeSet<_>>();
    if actual_non_python != expected_non_python {
        failures.push(json!({
            "corpus": "non_python",
            "error": "language set mismatch",
            "expected": expected_non_python,
            "actual": actual_non_python,
        }));
    }
    let by_language = non_python_stats
        .get("by_language")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for language in non_python_languages() {
        let count = by_language
            .get(language.slug())
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        if count < args.min_entries_per_non_python_language {
            failures.push(json!({
                "corpus": "non_python",
                "language": language.slug(),
                "error": "entry count below Phase C minimum",
                "actual": count,
                "minimum": args.min_entries_per_non_python_language,
            }));
        }
    }
    let toolchain_report_count =
        count_files_with_extension(&args.non_python_corpus.join("toolchain-reports"), "json")?;
    if toolchain_report_count != non_python_index.entries.len() {
        failures.push(json!({
            "corpus": "non_python",
            "error": "toolchain report count does not match index entries",
            "toolchain_report_count": toolchain_report_count,
            "index_entries": non_python_index.entries.len(),
        }));
    }

    let combined_split_entries = combined_split_entries(&python_index, &non_python_index)?;
    let combined_split = split_corpus(
        &combined_split_entries,
        SplitConfig {
            mode: SplitMode::InstanceAtomic,
            seed: 17,
            ..SplitConfig::default()
        },
    )?;
    let mut sha_by_task = load_python_test_patch_sha_map(&args.tasks_dir)?;
    for entry in &non_python_index.entries {
        if entry.category == MutationCategory::KnownGood {
            sha_by_task.insert(entry.task_id.clone(), entry.patch_sha256.clone());
        }
    }
    let split_leakage_check =
        assert_no_test_patch_leakage(&combined_split.assignments, |task_id| {
            sha_by_task.get(task_id).map(String::as_str)
        });
    if let Err(err) = &split_leakage_check {
        failures.push(json!({
            "corpus": "combined",
            "error": "combined split leakage check failed",
            "detail": err.to_string(),
            "code": err.code(),
        }));
    }

    let passes = failures.is_empty();
    let evidence = json!({
        "source_of_truth": {
            "python_index": args.python_corpus.join("index.json").display().to_string(),
            "python_store": python_store_path.display().to_string(),
            "python_verify_evidence": args.python_corpus.join("verify-evidence.json").display().to_string(),
            "non_python_index": args.non_python_corpus.join("index.json").display().to_string(),
            "non_python_store": non_python_store_path.display().to_string(),
            "non_python_verify_evidence": args.non_python_corpus.join("verify-evidence.json").display().to_string(),
            "non_python_toolchain_reports": args.non_python_corpus.join("toolchain-reports").display().to_string(),
            "combined_split_evidence": args.evidence.display().to_string(),
        },
        "python": {
            "entry_count": python_index.entries.len(),
            "mutation_cf_count": python_store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?,
            "verdict_cf_count": python_store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?,
            "verify_passes": python_verify.get("passes").and_then(Value::as_bool),
            "stats": python_stats,
        },
        "non_python": {
            "entry_count": non_python_index.entries.len(),
            "toolchain_report_count": toolchain_report_count,
            "mutation_cf_count": non_python_store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?,
            "verdict_cf_count": non_python_store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?,
            "verify_passes": non_python_verify.get("passes").and_then(Value::as_bool),
            "stats": non_python_stats,
        },
        "combined_split": {
            "task_count": combined_split.assignments.len(),
            "bucket_sizes": combined_split.bucket_sizes,
            "bucket_fractions": combined_split.bucket_fractions,
            "leakage_check_passed": combined_split.leakage_check_passed && split_leakage_check.is_ok(),
            "assignments": combined_split.assignments,
        },
        "failures": failures,
        "passes": passes,
    });
    write_json_checked(&args.evidence, &evidence)?;
    let readback: Value = serde_json::from_str(&fs::read_to_string(&args.evidence)?)?;
    if readback != evidence {
        return Err("Phase C evidence readback mismatch immediately after write".into());
    }
    if !passes {
        return Err(format!(
            "Phase C verification failed; evidence written to {}",
            args.evidence.display()
        )
        .into());
    }
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn read_json_required(path: &Path) -> CliResult<Value> {
    let text = fs::read_to_string(path).map_err(|err| {
        format!(
            "required JSON evidence file is not readable: {}; error={err}",
            path.display()
        )
    })?;
    serde_json::from_str(&text).map_err(|err| {
        format!(
            "required JSON evidence file is invalid JSON: {}; error={err}",
            path.display()
        )
        .into()
    })
}

fn count_files_with_extension(root: &Path, extension: &str) -> CliResult<usize> {
    if !root.is_dir() {
        return Err(format!(
            "required directory for extension count does not exist: {}",
            root.display()
        )
        .into());
    }
    let mut count = 0usize;
    count_files_with_extension_inner(root, extension, &mut count)?;
    Ok(count)
}

fn count_files_with_extension_inner(
    root: &Path,
    extension: &str,
    count: &mut usize,
) -> CliResult<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            count_files_with_extension_inner(&path, extension, count)?;
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            *count += 1;
        }
    }
    Ok(())
}

fn combined_split_entries(
    python_index: &CorpusIndex,
    non_python_index: &CorpusIndex,
) -> CliResult<Vec<CorpusEntry>> {
    let mut seen = BTreeSet::<String>::new();
    let mut entries = Vec::with_capacity(
        python_index.split_assignments.len() + non_python_index.split_assignments.len(),
    );
    for assignment in python_index
        .split_assignments
        .iter()
        .chain(non_python_index.split_assignments.iter())
    {
        if !seen.insert(assignment.task_id.clone()) {
            return Err(format!(
                "duplicate task_id in combined Phase C split: {}",
                assignment.task_id
            )
            .into());
        }
        entries.push(CorpusEntry {
            task_id: assignment.task_id.clone(),
            repo: assignment.repo.clone(),
        });
    }
    Ok(entries)
}

fn load_python_test_patch_sha_map(tasks_dir: &Path) -> CliResult<BTreeMap<String, String>> {
    let manifests = load_task_manifests(tasks_dir)?;
    Ok(manifests
        .into_iter()
        .map(|manifest| (manifest.instance_id, manifest.official_test_patch_sha256))
        .collect())
}

fn smoke(mut args: SmokeArgs) -> CliResult<()> {
    args.output = require_corpus_path(&args.output, "smoke.output")?;
    args.venv_python = require_runtime_python_path(&args.venv_python, "smoke.venv_python")?;
    // Same ephemeral-on-ext4 pattern as `generate` (see comments there) —
    // smoke is just generate-with-tighter-scope, so the cache/work paths
    // have identical durability semantics.
    args.repo_cache_dir = require_ephemeral_path(&args.repo_cache_dir, "smoke.repo_cache_dir")?;
    args.source_work_root =
        require_ephemeral_path(&args.source_work_root, "smoke.source_work_root")?;
    validate_smoke_args(&args)?;
    let task_limit = args.max_tasks.or({
        if args.instance_ids.is_empty() {
            Some(1)
        } else {
            None
        }
    });
    let generate_args = GenerateArgs {
        output: args.output.clone(),
        tasks_dir: args.tasks_dir.clone(),
        dataset_name: args.dataset_name.clone(),
        dataset_split: args.dataset_split.clone(),
        venv_python: args.venv_python.clone(),
        instance_ids: args.instance_ids.clone(),
        categories: args.categories.clone(),
        languages: args.languages.clone(),
        seed: args.seed,
        max_tasks: task_limit,
        skip_tasks: args.skip_tasks,
        repo_cache_dir: args.repo_cache_dir.clone(),
        source_work_root: args.source_work_root.clone(),
        run_id_prefix: args.run_id_prefix.clone(),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
        oracle_workers: args.oracle_workers,
        patch_workers: 0,
        resume_incomplete: args.resume_incomplete,
    };
    let verify_args = VerifyArgs {
        corpus: args.output.clone(),
        sample_fraction: args.sample_fraction,
        sample_keys: Vec::new(),
        quarantine_config: None,
        venv_python: args.venv_python.clone(),
        run_id_prefix: format!("{}-verify", args.run_id_prefix),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
        oracle_repeat_runs: 1,
        oracle_repeat_output: None,
    };

    let mut failures = Vec::<Value>::new();
    let generate_started = Instant::now();
    if let Err(err) = generate(generate_args) {
        failures.push(json!({
            "phase": "generate",
            "error": err.to_string(),
        }));
        return finish_smoke(args, failures, generate_started.elapsed(), None, None);
    }
    let generate_elapsed = generate_started.elapsed();

    let verify_started = Instant::now();
    if let Err(err) = verify(verify_args) {
        failures.push(json!({
            "phase": "verify",
            "error": err.to_string(),
        }));
    }
    let verify_elapsed = verify_started.elapsed();

    let verify_evidence = read_optional_json(&args.output.join("verify-evidence.json"));
    finish_smoke(
        args,
        failures,
        generate_elapsed,
        Some(verify_elapsed),
        verify_evidence,
    )
}

fn finish_smoke(
    args: SmokeArgs,
    mut failures: Vec<Value>,
    generate_elapsed: Duration,
    verify_elapsed: Option<Duration>,
    verify_evidence: Option<Value>,
) -> CliResult<()> {
    let index = read_index(&args.output).ok();
    let mut stats_json = None;
    let mut stats_error = None;
    let mut store_path = None;
    if let Some(index) = &index {
        match resolve_corpus_path(&args.output, &index.store_path).and_then(|path| {
            let store = OracleStore::open(&path)?;
            let stats = build_stats_json(&args.output, index, &store)?;
            Ok((path, stats))
        }) {
            Ok((path, stats)) => {
                store_path = Some(path);
                let expected_entries = index.entries.len();
                if !stats_source_of_truth_passes(&stats, expected_entries) {
                    failures.push(json!({
                        "phase": "stats",
                        "error": "stats source-of-truth integrity check did not pass",
                        "expected_entry_count": expected_entries,
                        "stats": stats,
                    }));
                }
                stats_json = Some(stats);
            }
            Err(err) => {
                let message = err.to_string();
                stats_error = Some(message.clone());
                failures.push(json!({
                    "phase": "stats",
                    "error": message,
                }));
            }
        }
    }

    let entry_count = index.as_ref().map(|index| index.entries.len()).unwrap_or(0);
    let task_count = index
        .as_ref()
        .map(|index| {
            index
                .entries
                .iter()
                .map(|entry| entry.task_id.as_str())
                .collect::<BTreeSet<_>>()
                .len()
        })
        .unwrap_or(0);
    let category_count = index
        .as_ref()
        .map(|index| {
            index
                .entries
                .iter()
                .map(|entry| entry.category.slug())
                .collect::<BTreeSet<_>>()
                .len()
        })
        .unwrap_or(0);
    let generate_secs = duration_secs(generate_elapsed);
    let verify_secs = verify_elapsed.map(duration_secs);
    let generate_secs_per_entry = if entry_count == 0 {
        None
    } else {
        Some(generate_secs / entry_count as f64)
    };
    if let Some(avg) = generate_secs_per_entry {
        if avg > args.max_generate_secs_per_entry {
            failures.push(json!({
                "phase": "performance",
                "metric": "generate_secs_per_entry",
                "actual": avg,
                "max_allowed": args.max_generate_secs_per_entry,
            }));
        }
    }
    let oracle_sample_count = verify_evidence
        .as_ref()
        .and_then(|value| value.get("oracle_sample_count"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if let (Some(total), true) = (verify_secs, oracle_sample_count > 0) {
        let per_sample = total / oracle_sample_count as f64;
        if per_sample > args.max_verify_secs_per_sample {
            failures.push(json!({
                "phase": "performance",
                "metric": "verify_secs_per_oracle_sample",
                "actual": per_sample,
                "max_allowed": args.max_verify_secs_per_sample,
            }));
        }
    }

    let zero_per_test_entries = index
        .as_ref()
        .map(|index| {
            index
                .entries
                .iter()
                .filter(|entry| entry.oracle_per_test_count == 0)
                .map(|entry| {
                    json!({
                        "task_id": entry.task_id,
                        "category": entry.category.slug(),
                        "exception": entry.oracle_exception,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let known_good_failures = index
        .as_ref()
        .map(|index| {
            index
                .entries
                .iter()
                .filter(|entry| entry.category == MutationCategory::KnownGood)
                .filter(|entry| !entry.oracle_all_passed)
                .map(|entry| {
                    json!({
                        "task_id": entry.task_id,
                        "oracle_exception": entry.oracle_exception,
                        "oracle_per_test_count": entry.oracle_per_test_count,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if !known_good_failures.is_empty() {
        failures.push(json!({
            "phase": "accuracy",
            "error": "known_good mutation did not pass the SWE-bench oracle",
            "entries": known_good_failures,
        }));
    }

    let passes = failures.is_empty();
    let evidence_path = args.output.join("smoke-evidence.json");
    let evidence = json!({
        "passes": passes,
        "source_of_truth": {
            "index": args.output.join("index.json").display().to_string(),
            "store": store_path.as_ref().map(|path| path.display().to_string()),
            "verify_evidence": args.output.join("verify-evidence.json").display().to_string(),
            "stats_json": args.output.join("stats.json").display().to_string(),
            "smoke_evidence": evidence_path.display().to_string(),
        },
        "phase0_language_scope": {
            "full_corpus_language": "python",
            "requires_all_ast_languages_before_full_corpus": false,
            "reason": "SWE-bench Lite oracle smoke remains Python-only; Phase C non-Python fixtures use generate-fixtures and real language toolchains."
        },
        "config": {
            "tasks_dir": args.tasks_dir,
            "requested_instance_ids": args.instance_ids,
            "defaulted_max_tasks": args.max_tasks.is_none(),
            "max_tasks": args.max_tasks,
            "categories": args.categories,
            "languages": args.languages,
            "seed": args.seed,
            "sample_fraction": args.sample_fraction,
            "oracle_workers": args.oracle_workers,
            "max_generate_secs_per_entry": args.max_generate_secs_per_entry,
            "max_verify_secs_per_sample": args.max_verify_secs_per_sample,
        },
        "coverage": {
            "task_count": task_count,
            "category_count": category_count,
            "entry_count": entry_count,
            "zero_per_test_entries": zero_per_test_entries,
            "known_good_failures": known_good_failures,
        },
        "performance": {
            "generate_secs": generate_secs,
            "verify_secs": verify_secs,
            "generate_secs_per_entry": generate_secs_per_entry,
            "verify_oracle_sample_count": oracle_sample_count,
            "verify_secs_per_oracle_sample": verify_secs.and_then(|secs| {
                if oracle_sample_count == 0 {
                    None
                } else {
                    Some(secs / oracle_sample_count as f64)
                }
            }),
            "estimated_full_phase0_entries": 300 * MutationCategory::all().len(),
            "estimated_full_phase0_generate_secs_from_smoke": generate_secs_per_entry
                .map(|per_entry| per_entry * (300 * MutationCategory::all().len()) as f64),
        },
        "verify_evidence": verify_evidence,
        "stats": stats_json,
        "stats_error": stats_error,
        "failures": failures,
    });
    write_json_checked(&evidence_path, &evidence)?;
    if !passes {
        return Err(format!(
            "smoke failed; evidence written to {}",
            evidence_path.display()
        )
        .into());
    }
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn validate_smoke_args(args: &SmokeArgs) -> CliResult<()> {
    if args.max_generate_secs_per_entry <= 0.0 || !args.max_generate_secs_per_entry.is_finite() {
        return Err("max_generate_secs_per_entry must be finite and > 0".into());
    }
    if args.max_verify_secs_per_sample <= 0.0 || !args.max_verify_secs_per_sample.is_finite() {
        return Err("max_verify_secs_per_sample must be finite and > 0".into());
    }
    validate_generate_args(&GenerateArgs {
        output: args.output.clone(),
        tasks_dir: args.tasks_dir.clone(),
        dataset_name: args.dataset_name.clone(),
        dataset_split: args.dataset_split.clone(),
        venv_python: args.venv_python.clone(),
        instance_ids: args.instance_ids.clone(),
        categories: args.categories.clone(),
        languages: args.languages.clone(),
        seed: args.seed,
        max_tasks: args.max_tasks,
        skip_tasks: args.skip_tasks,
        repo_cache_dir: args.repo_cache_dir.clone(),
        source_work_root: args.source_work_root.clone(),
        run_id_prefix: args.run_id_prefix.clone(),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
        oracle_workers: args.oracle_workers,
        patch_workers: 0,
        resume_incomplete: args.resume_incomplete,
    })?;
    validate_verify_args(&VerifyArgs {
        corpus: args.output.clone(),
        sample_fraction: args.sample_fraction,
        sample_keys: Vec::new(),
        quarantine_config: None,
        venv_python: args.venv_python.clone(),
        run_id_prefix: format!("{}-verify", args.run_id_prefix),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
        oracle_repeat_runs: 1,
        oracle_repeat_output: None,
    })?;
    Ok(())
}

fn require_durable_path(path: &Path, field: &'static str) -> CliResult<PathBuf> {
    context_graph_paths::require_under_data_root(path, field).map_err(|err| err.to_string().into())
}

fn require_runtime_python_path(path: &Path, field: &'static str) -> CliResult<PathBuf> {
    context_graph_paths::require_under_data_root(path, field)
        .or_else(|_| context_graph_paths::require_production_hot_root(path, field))
        .map_err(|err| err.to_string().into())
}

/// CLI-side validator for **ephemeral** paths (caches, scratch work). See
/// `context_graph_paths::require_ephemeral_path` for the contract. Used by
/// the `generate` command for `--repo-cache-dir` and `--source-work-root`,
/// which on WSL2 must live on ext4 for acceptable throughput.
fn require_ephemeral_path(path: &Path, field: &'static str) -> CliResult<PathBuf> {
    context_graph_paths::require_ephemeral_path(path, field).map_err(|err| err.to_string().into())
}

fn require_corpus_path(path: &Path, field: &'static str) -> CliResult<PathBuf> {
    context_graph_paths::require_under_mejepa_corpus_root(path, field)
        .map_err(|err| err.to_string().into())
}

fn validate_generate_args(args: &GenerateArgs) -> CliResult<()> {
    if args.dataset_name.trim().is_empty() {
        return Err("dataset_name must be non-empty".into());
    }
    if args.dataset_split.trim().is_empty() {
        return Err("dataset_split must be non-empty".into());
    }
    if !args.venv_python.is_file() {
        return Err(format!(
            "SWE-bench venv python does not exist: {}",
            args.venv_python.display()
        )
        .into());
    }
    if args.instance_timeout_secs == 0 || args.overall_timeout_secs < args.instance_timeout_secs {
        return Err(
            "timeouts invalid: require overall_timeout_secs >= instance_timeout_secs > 0".into(),
        );
    }
    if args.run_id_prefix.trim().is_empty() {
        return Err("run_id_prefix must be non-empty".into());
    }
    if args.oracle_workers == 0 {
        return Err("oracle_workers must be > 0".into());
    }
    if args.max_tasks == Some(0) {
        return Err("max_tasks must be > 0 when provided".into());
    }
    if args.skip_tasks > 0 && !args.instance_ids.is_empty() {
        return Err("skip_tasks cannot be combined with explicit --instance-id selection".into());
    }
    validate_docker_run_id_component(&args.run_id_prefix)?;
    Ok(())
}

fn validate_prepare_images_args(args: &PrepareImagesArgs) -> CliResult<()> {
    validate_docker_namespace(&args.namespace, "prepare-images namespace")?;
    if args.parallel == 0 {
        return Err("prepare-images parallel must be > 0".into());
    }
    if args.pull_timeout_secs == 0 {
        return Err("prepare-images pull_timeout_secs must be > 0".into());
    }
    if args.max_tasks == Some(0) {
        return Err("prepare-images max_tasks must be > 0 when provided".into());
    }
    if args.skip_tasks > 0 && !args.instance_ids.is_empty() {
        return Err("prepare-images skip_tasks cannot be combined with --instance-id".into());
    }
    Ok(())
}

fn validate_cleanup_args(args: &CleanupArgs) -> CliResult<()> {
    validate_docker_namespace(&args.namespace, "cleanup namespace")
}

fn validate_normalize_args(args: &NormalizeArgs) -> CliResult<()> {
    if args.corpus == args.output {
        return Err(
            "normalize requires --output to be a fresh directory distinct from --corpus".into(),
        );
    }
    if !args.corpus.is_dir() {
        return Err(format!(
            "normalize corpus does not exist or is not a directory: {}",
            args.corpus.display()
        )
        .into());
    }
    ensure_output_directory_ready(&args.output)
}

fn validate_prune_quarantine_args(args: &PruneQuarantineArgs) -> CliResult<()> {
    if args.corpus == args.output {
        return Err(
            "prune-quarantine requires --output to be a fresh directory distinct from --corpus"
                .into(),
        );
    }
    if !args.corpus.is_dir() {
        return Err(format!(
            "prune-quarantine corpus does not exist or is not a directory: {}",
            args.corpus.display()
        )
        .into());
    }
    if !args.quarantine_config.is_file() {
        return Err(format!(
            "prune-quarantine quarantine config does not exist: {}",
            args.quarantine_config.display()
        )
        .into());
    }
    ensure_output_directory_ready(&args.output)
}

fn validate_docker_namespace(namespace: &str, field: &'static str) -> CliResult<()> {
    if namespace.trim().is_empty() {
        return Err(format!("{field} must be non-empty").into());
    }
    if namespace
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(format!("{field} cannot contain whitespace or control characters").into());
    }
    if namespace.contains("..") || namespace.starts_with('/') {
        return Err(format!("{field} must be a Docker namespace, not a path").into());
    }
    Ok(())
}

fn pull_images_bounded(
    images: Vec<String>,
    parallel: usize,
    timeout: Duration,
) -> CliResult<Vec<Value>> {
    if images.is_empty() {
        return Ok(Vec::new());
    }
    let images = Arc::new(images);
    let cursor = Arc::new(AtomicUsize::new(0));
    let results = Arc::new(Mutex::new(Vec::new()));
    let worker_count = parallel.min(images.len());
    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let images = Arc::clone(&images);
        let cursor = Arc::clone(&cursor);
        let results = Arc::clone(&results);
        handles.push(thread::spawn(move || loop {
            let index = cursor.fetch_add(1, Ordering::SeqCst);
            let Some(image) = images.get(index) else {
                break;
            };
            let result = pull_one_image(image, timeout);
            let mut guard = results.lock().expect("image-prep result mutex poisoned");
            guard.push(result);
        }));
    }
    for handle in handles {
        handle
            .join()
            .map_err(|_| "image preparation worker thread panicked")?;
    }
    let mut results = Arc::try_unwrap(results)
        .map_err(|_| "image preparation results still have live references")?
        .into_inner()
        .map_err(|_| "image preparation result mutex poisoned")?;
    results.sort_by(|a, b| {
        a.get("image")
            .and_then(Value::as_str)
            .cmp(&b.get("image").and_then(Value::as_str))
    });
    Ok(results)
}

fn pull_one_image(image: &str, timeout: Duration) -> Value {
    let started = Instant::now();
    let logs = match pull_log_paths(image) {
        Ok(logs) => logs,
        Err(error) => {
            return json!({
                "image": image,
                "success": false,
                "exit_code": null,
                "duration_secs": duration_secs(started.elapsed()),
                "error": error.to_string(),
            });
        }
    };
    let mut command = Command::new("docker");
    let stdout_file = match File::create(&logs.stdout) {
        Ok(file) => file,
        Err(error) => {
            return json!({
                "image": image,
                "success": false,
                "exit_code": null,
                "duration_secs": duration_secs(started.elapsed()),
                "stdout_log": logs.stdout,
                "stderr_log": logs.stderr,
                "error": format!("failed to create docker pull stdout log: {error}"),
            });
        }
    };
    let stderr_file = match File::create(&logs.stderr) {
        Ok(file) => file,
        Err(error) => {
            return json!({
                "image": image,
                "success": false,
                "exit_code": null,
                "duration_secs": duration_secs(started.elapsed()),
                "stdout_log": logs.stdout,
                "stderr_log": logs.stderr,
                "error": format!("failed to create docker pull stderr log: {error}"),
            });
        }
    };
    command
        .args(["pull", "--quiet", image])
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    configure_child_process_group(&mut command);
    let result = command
        .spawn()
        .map_err(|err| format!("failed to spawn docker pull: {err}"))
        .and_then(|child| {
            wait_for_child_status(child, timeout, "docker pull").map_err(|err| err.to_string())
        });
    let stdout_tail = tail_file(&logs.stdout);
    let stderr_tail = tail_file(&logs.stderr);
    match result {
        Ok(status) => json!({
            "image": image,
            "success": status.success(),
            "exit_code": status.code(),
            "duration_secs": duration_secs(started.elapsed()),
            "stdout_log": logs.stdout,
            "stderr_log": logs.stderr,
            "stdout_tail": stdout_tail.unwrap_or_else(|err| format!("<failed to read stdout log: {err}>")),
            "stderr_tail": stderr_tail.unwrap_or_else(|err| format!("<failed to read stderr log: {err}>")),
        }),
        Err(error) => json!({
            "image": image,
            "success": false,
            "exit_code": null,
            "duration_secs": duration_secs(started.elapsed()),
            "stdout_log": logs.stdout,
            "stderr_log": logs.stderr,
            "stdout_tail": stdout_tail.unwrap_or_else(|err| format!("<failed to read stdout log: {err}>")),
            "stderr_tail": stderr_tail.unwrap_or_else(|err| format!("<failed to read stderr log: {err}>")),
            "error": error,
        }),
    }
}

#[derive(Debug, Clone)]
struct PullLogPaths {
    stdout: PathBuf,
    stderr: PathBuf,
}

fn pull_log_paths(image: &str) -> CliResult<PullLogPaths> {
    let digest = format!("{:x}", Sha256::digest(image.as_bytes()));
    let base = context_graph_paths::mejepa_image_prep_log_dir()?;
    Ok(PullLogPaths {
        stdout: base.join(format!("docker-pull-{digest}.stdout.log")),
        stderr: base.join(format!("docker-pull-{digest}.stderr.log")),
    })
}

fn configure_child_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}

fn wait_for_child_status(
    mut child: Child,
    timeout: Duration,
    label: &str,
) -> CliResult<ExitStatus> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            terminate_child_process_group(&mut child)?;
            let _ = child.wait()?;
            return Err(format!(
                "{label} timed out after {timeout:?}; process group was terminated and reaped",
            )
            .into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn terminate_child_process_group(child: &mut Child) -> CliResult<()> {
    #[cfg(unix)]
    {
        let process_group = format!("-{}", child.id());
        let term = Command::new("kill")
            .args(["-TERM", "--", &process_group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !term.success() {
            let _ = child.kill();
        }
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(2) {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(25));
        }
        let kill = Command::new("kill")
            .args(["-KILL", "--", &process_group])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if !kill.success() {
            let _ = child.kill();
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        child.kill()?;
        Ok(())
    }
}

fn swebench_instance_image_ref(namespace: &str, instance_id: &str) -> CliResult<String> {
    let (owner, rest) = instance_id
        .split_once("__")
        .ok_or_else(|| format!("invalid SWE-bench instance_id for image cleanup: {instance_id}"))?;
    if owner.is_empty() || rest.is_empty() {
        return Err(
            format!("invalid SWE-bench instance_id for image cleanup: {instance_id}").into(),
        );
    }
    Ok(format!(
        "{namespace}/sweb.eval.x86_64.{owner}_1776_{rest}:latest"
    ))
}

fn docker_stdout(args: &[&str]) -> CliResult<String> {
    let output = run_capture_timed(
        Path::new("docker"),
        args,
        DOCKER_QUERY_TIMEOUT,
        "docker stdout command",
    )
    .map_err(|err| {
        format!(
            "docker {} failed before exit: {err} ({})",
            args.join(" "),
            err.code()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "docker {} failed: status={:?} stdout_tail={} stderr_tail={}",
            args.join(" "),
            output.status.code(),
            tail_bytes(&output.stdout),
            tail_bytes(&output.stderr),
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn docker_status(args: &[&str]) -> CliResult<()> {
    let output = run_capture_timed(
        Path::new("docker"),
        args,
        DOCKER_QUERY_TIMEOUT,
        "docker status command",
    )
    .map_err(|err| {
        format!(
            "docker {} failed before exit: {err} ({})",
            args.join(" "),
            err.code()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "docker {} failed: status={:?} stdout_tail={} stderr_tail={}",
            args.join(" "),
            output.status.code(),
            tail_bytes(&output.stdout),
            tail_bytes(&output.stderr),
        )
        .into());
    }
    Ok(())
}

fn tail_bytes(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines().rev().take(20).collect::<Vec<_>>();
    lines.reverse();
    lines.join("\n")
}

fn tail_file(path: &Path) -> CliResult<String> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(tail_bytes(&bytes))
}

fn validate_verify_args(args: &VerifyArgs) -> CliResult<()> {
    if !args.venv_python.is_file() {
        return Err(format!(
            "SWE-bench venv python does not exist: {}",
            args.venv_python.display()
        )
        .into());
    }
    if !(args.sample_fraction.is_finite()
        && args.sample_fraction > 0.0
        && args.sample_fraction <= 1.0)
    {
        return Err(format!(
            "sample_fraction must be finite and in the interval (0, 1], got {}",
            args.sample_fraction
        )
        .into());
    }
    if args.instance_timeout_secs == 0 || args.overall_timeout_secs < args.instance_timeout_secs {
        return Err(
            "timeouts invalid: require overall_timeout_secs >= instance_timeout_secs > 0".into(),
        );
    }
    if args.run_id_prefix.trim().is_empty() {
        return Err("run_id_prefix must be non-empty".into());
    }
    validate_docker_run_id_component(&args.run_id_prefix)?;
    if args.oracle_repeat_runs == 0 {
        return Err("oracle_repeat_runs must be >= 1".into());
    }
    if args.oracle_repeat_output.is_some() && args.oracle_repeat_runs < 2 {
        return Err("oracle_repeat_runs must be >= 2 when oracle_repeat_output is set".into());
    }
    Ok(())
}

fn load_corpus_quarantine(path: &Path) -> CliResult<BTreeMap<String, CorpusQuarantineEntry>> {
    if !path.is_file() {
        return Err(format!("MEJEPA_CORPUS_QUARANTINE_MISSING: {}", path.display()).into());
    }
    let raw = fs::read_to_string(path)?;
    let config: CorpusQuarantineConfig = toml::from_str(&raw)?;
    let mut entries = BTreeMap::new();
    for entry in config.quarantined_tasks {
        validate_corpus_quarantine_entry(&entry)?;
        let task_id = entry.task_id.clone();
        if entries.insert(task_id.clone(), entry).is_some() {
            return Err(format!("MEJEPA_CORPUS_QUARANTINE_DUPLICATE_TASK: {}", task_id).into());
        }
    }
    Ok(entries)
}

fn validate_corpus_quarantine_entry(entry: &CorpusQuarantineEntry) -> CliResult<()> {
    validate_corpus_quarantine_task_id(&entry.task_id)?;
    if entry.reason.trim().is_empty() {
        return Err(format!("MEJEPA_CORPUS_QUARANTINE_REASON_MISSING: {}", entry.task_id).into());
    }
    if !(entry.flakiness_rate.is_finite() && (0.0..=1.0).contains(&entry.flakiness_rate)) {
        return Err(format!("MEJEPA_CORPUS_QUARANTINE_RATE_INVALID: {}", entry.task_id).into());
    }
    if entry.oracle_runs < 2 {
        return Err(format!(
            "MEJEPA_CORPUS_QUARANTINE_INSUFFICIENT_RUNS: {}",
            entry.task_id
        )
        .into());
    }
    if entry.observed_outcomes.len() != entry.oracle_runs {
        return Err(format!(
            "MEJEPA_CORPUS_QUARANTINE_OUTCOME_COUNT_MISMATCH: {}",
            entry.task_id
        )
        .into());
    }
    if entry.observed_verdict_sha256.len() != entry.oracle_runs {
        return Err(format!(
            "MEJEPA_CORPUS_QUARANTINE_HASH_COUNT_MISMATCH: {}",
            entry.task_id
        )
        .into());
    }
    for hash in &entry.observed_verdict_sha256 {
        normalize_corpus_quarantine_sha256(hash)?;
    }
    if entry.operator_id.trim().is_empty() {
        return Err(format!(
            "MEJEPA_CORPUS_QUARANTINE_OPERATOR_MISSING: {}",
            entry.task_id
        )
        .into());
    }
    if entry.created_unix_ms <= 0 {
        return Err(format!(
            "MEJEPA_CORPUS_QUARANTINE_CREATED_AT_INVALID: {}",
            entry.task_id
        )
        .into());
    }
    Ok(())
}

fn validate_corpus_quarantine_task_id(task_id: &str) -> CliResult<()> {
    if task_id.trim().is_empty() || task_id.trim() != task_id {
        return Err("MEJEPA_CORPUS_QUARANTINE_TASK_ID_INVALID".into());
    }
    if task_id
        .bytes()
        .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    {
        return Err("MEJEPA_CORPUS_QUARANTINE_TASK_ID_INVALID".into());
    }
    Ok(())
}

fn normalize_corpus_quarantine_sha256(value: &str) -> CliResult<String> {
    let stripped = value.strip_prefix("sha256:").unwrap_or(value);
    if stripped.len() == 64 && stripped.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(stripped.to_ascii_lowercase())
    } else {
        Err(format!("MEJEPA_CORPUS_QUARANTINE_HASH_INVALID: {value}").into())
    }
}

fn ensure_output_directory_ready(output: &Path) -> CliResult<()> {
    if !output.exists() {
        return Ok(());
    }
    if !output.is_dir() {
        return Err(format!(
            "output path exists but is not a directory: {}",
            output.display()
        )
        .into());
    }
    let mut entries = fs::read_dir(output)?;
    if entries.next().transpose()?.is_some() {
        return Err(format!(
            "output directory is not empty: {}; choose a fresh corpus directory so stale RocksDB rows cannot contaminate verification",
            output.display()
        )
        .into());
    }
    Ok(())
}

fn ensure_resume_directory_ready(output: &Path) -> CliResult<()> {
    if !output.is_dir() {
        return Err(format!(
            "resume output directory does not exist or is not a directory: {}",
            output.display()
        )
        .into());
    }
    let index_path = output.join("index.json");
    if !index_path.is_file() {
        return Err(format!(
            "resume requires an existing incomplete index at {}",
            index_path.display()
        )
        .into());
    }
    Ok(())
}

fn validate_resume_index(
    index: &CorpusIndex,
    args: &GenerateArgs,
    languages: &[Language],
    split_assignments: &[TaskAssignment],
    store_path_rel: &Path,
    selected_task_ids: &[String],
) -> CliResult<()> {
    if index.complete {
        return Err("cannot resume a corpus whose index is already complete=true".into());
    }
    if index.schema_version != 1 {
        return Err(format!(
            "cannot resume unsupported corpus index schema_version={}",
            index.schema_version
        )
        .into());
    }
    if index.corpus_version != MEJEPA_CORPUS_VERSION_V1 {
        return Err(format!(
            "cannot resume corpus_version={} with generator corpus_version={}",
            index.corpus_version, MEJEPA_CORPUS_VERSION_V1
        )
        .into());
    }
    if index.embedder_version != MEJEPA_EMBEDDER_VERSION_V1 {
        return Err(format!(
            "cannot resume embedder_version={} with generator embedder_version={}",
            index.embedder_version, MEJEPA_EMBEDDER_VERSION_V1
        )
        .into());
    }
    if index.dataset_name != args.dataset_name {
        return Err(format!(
            "cannot resume with different dataset_name: index={} args={}",
            index.dataset_name, args.dataset_name
        )
        .into());
    }
    if index.dataset_split != args.dataset_split {
        return Err(format!(
            "cannot resume with different dataset_split: index={} args={}",
            index.dataset_split, args.dataset_split
        )
        .into());
    }
    if index.seed != args.seed {
        return Err(format!(
            "cannot resume with different seed: index={} args={}",
            index.seed, args.seed
        )
        .into());
    }
    if index.languages != languages {
        return Err(format!(
            "cannot resume with different languages: index={:?} args={:?}",
            index.languages, languages
        )
        .into());
    }
    if index.tasks_dir != args.tasks_dir.display().to_string() {
        return Err(format!(
            "cannot resume with different tasks_dir: index={} args={}",
            index.tasks_dir,
            args.tasks_dir.display()
        )
        .into());
    }
    if index.store_path != store_path_rel.display().to_string() {
        return Err(format!(
            "cannot resume with different store_path: index={} expected={}",
            index.store_path,
            store_path_rel.display()
        )
        .into());
    }
    if index.corpus_sha256.is_some() {
        return Err("incomplete resume index must not already have corpus_sha256".into());
    }
    if index.split_assignments != split_assignments {
        return Err("cannot resume because split assignments changed".into());
    }
    if !index.selected_task_ids.is_empty() && index.selected_task_ids != selected_task_ids {
        return Err(format!(
            "cannot resume with different task selection: index has {} task ids, args select {} task ids",
            index.selected_task_ids.len(),
            selected_task_ids.len()
        )
        .into());
    }
    Ok(())
}

fn validate_resume_rows(
    output: &Path,
    index: &CorpusIndex,
    store: &OracleStore,
) -> CliResult<BTreeSet<String>> {
    let mut keys = BTreeSet::new();
    validate_resume_patch_file_set(output, index)?;
    for entry in &index.entries {
        let key = corpus_row_key(&entry.task_id, entry.category);
        if !keys.insert(key.clone()) {
            return Err(format!("cannot resume duplicate index row: {key}").into());
        }
        let patch_path = resolve_corpus_path(output, &entry.patch_path)?;
        let patch_text = fs::read_to_string(&patch_path)?;
        let patch_hash = sha256_text(&patch_text);
        if patch_hash != entry.patch_sha256 {
            return Err(format!(
                "cannot resume because patch hash changed for {key}: expected {} actual {}",
                entry.patch_sha256, patch_hash
            )
            .into());
        }
        let (mutation, verdict) = store
            .get_corpus_row(&entry.task_id, entry.category)?
            .ok_or_else(|| format!("cannot resume because RocksDB row is missing for {key}"))?;
        if sha256_text(&mutation.mutated_source) != entry.patch_sha256 {
            return Err(format!("cannot resume because mutation hash mismatches for {key}").into());
        }
        let verdict_hash = sha256_json_value(&serde_json::to_value(&verdict)?);
        if verdict_hash != entry.oracle_verdict_sha256 {
            return Err(format!(
                "cannot resume because verdict hash mismatches for {key}: expected {} actual {}",
                entry.oracle_verdict_sha256, verdict_hash
            )
            .into());
        }
    }
    let store_keys: BTreeSet<String> = store
        .iter_corpus_rows()?
        .into_iter()
        .map(|(task_id, category, _mutation, _verdict)| corpus_row_key(&task_id, category))
        .collect();
    if store_keys != keys {
        return Err(format!(
            "cannot resume because RocksDB row set diverges from index: index_count={} store_count={} missing_in_store={:?} extra_in_store={:?}",
            keys.len(),
            store_keys.len(),
            sample_set_difference(&keys, &store_keys, 8),
            sample_set_difference(&store_keys, &keys, 8),
        )
        .into());
    }
    Ok(keys)
}

fn validate_resume_patch_file_set(output: &Path, index: &CorpusIndex) -> CliResult<()> {
    let expected: BTreeSet<String> = index
        .entries
        .iter()
        .map(|entry| entry.patch_path.clone())
        .collect();
    let actual = collect_patch_files(&output.join("mutations"), output)?;
    if actual != expected {
        return Err(format!(
            "cannot resume because mutation patch files diverge from index: index_count={} patch_file_count={} missing_files={:?} extra_files={:?}",
            expected.len(),
            actual.len(),
            sample_set_difference(&expected, &actual, 8),
            sample_set_difference(&actual, &expected, 8),
        )
        .into());
    }
    Ok(())
}

fn collect_patch_files(root: &Path, output: &Path) -> CliResult<BTreeSet<String>> {
    let mut files = BTreeSet::new();
    if !root.exists() {
        return Ok(files);
    }
    collect_patch_files_rec(root, output, &mut files)?;
    Ok(files)
}

fn collect_patch_files_rec(
    dir: &Path,
    output: &Path,
    files: &mut BTreeSet<String>,
) -> CliResult<()> {
    for entry in fs::read_dir(dir).map_err(|err| {
        format!(
            "failed to read mutation patch directory {}: {err}",
            dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_patch_files_rec(&path, output, files)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "patch") {
            let relative = path.strip_prefix(output).map_err(|err| {
                format!(
                    "mutation patch path {} is not under output {}: {err}",
                    path.display(),
                    output.display()
                )
            })?;
            files.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn sample_set_difference(
    left: &BTreeSet<String>,
    right: &BTreeSet<String>,
    limit: usize,
) -> Vec<String> {
    left.difference(right).take(limit).cloned().collect()
}

fn load_task_manifests(tasks_dir: &Path) -> CliResult<Vec<TaskManifest>> {
    let mut manifests = Vec::new();
    for entry in fs::read_dir(tasks_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let manifest: TaskManifest = serde_json::from_str(&fs::read_to_string(&path)?)
            .map_err(|err| format!("failed to parse task manifest {}: {err}", path.display()))?;
        manifests.push(manifest);
    }
    manifests.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    if manifests.is_empty() {
        return Err(format!("no task manifests found in {}", tasks_dir.display()).into());
    }
    Ok(manifests)
}

fn select_manifests(
    manifests: &[TaskManifest],
    instance_ids: &[String],
    skip_tasks: usize,
    max_tasks: Option<usize>,
) -> CliResult<Vec<TaskManifest>> {
    if skip_tasks > 0 && !instance_ids.is_empty() {
        return Err("skip_tasks cannot be combined with explicit --instance-id selection".into());
    }
    let selected = if instance_ids.is_empty() {
        manifests.to_vec()
    } else {
        let wanted = instance_ids.iter().collect::<BTreeSet<_>>();
        let selected = manifests
            .iter()
            .filter(|manifest| wanted.contains(&manifest.instance_id))
            .cloned()
            .collect::<Vec<_>>();
        if selected.len() != wanted.len() {
            return Err(format!(
                "selected {} manifests but {} instance ids were requested",
                selected.len(),
                wanted.len()
            )
            .into());
        }
        selected
    };
    let selected = selected
        .into_iter()
        .skip(skip_tasks)
        .take(max_tasks.unwrap_or(usize::MAX))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err("selection produced zero task manifests".into());
    }
    Ok(selected)
}

fn load_official_patches(
    venv_python: &Path,
    dataset_name: &str,
    dataset_split: &str,
    instance_ids: &[&str],
) -> CliResult<BTreeMap<String, String>> {
    let ids_json = serde_json::to_string(instance_ids)?;
    let snippet = r#"
import json
import sys
from datasets import load_dataset
ids = set(json.loads(sys.argv[1]))
dataset_name = sys.argv[2]
dataset_split = sys.argv[3]
rows = {}
for row in load_dataset(dataset_name, split=dataset_split):
    if row["instance_id"] in ids:
        rows[row["instance_id"]] = row.get("patch") or ""
missing = sorted(ids - set(rows))
if missing:
    raise SystemExit("missing official dataset rows: " + ",".join(missing))
print(json.dumps(rows, sort_keys=True))
"#;
    let output = run_capture_timed(
        venv_python,
        &["-c", snippet, &ids_json, dataset_name, dataset_split],
        OFFICIAL_PATCH_LOADER_TIMEOUT,
        "load official SWE-bench patches",
    )
    .map_err(|err| {
        format!(
            "official patch loader failed before exit: {err} ({})",
            err.code()
        )
    })?;
    if !output.status.success() {
        return Err(format!(
            "official patch loader failed: status={:?} stdout={} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let patches: BTreeMap<String, String> = serde_json::from_slice(&output.stdout)?;
    for (task, patch) in &patches {
        if patch.trim().is_empty() {
            return Err(format!("official dataset returned empty patch for {task}").into());
        }
    }
    Ok(patches)
}

fn parse_categories(values: &[String]) -> CliResult<Vec<MutationCategory>> {
    if values.is_empty() {
        return Ok(MutationCategory::all().to_vec());
    }
    values.iter().map(|value| parse_category(value)).collect()
}

fn parse_category(value: &str) -> CliResult<MutationCategory> {
    MutationCategory::from_slug(value)
        .ok_or_else(|| format!("unknown mutation category `{value}`").into())
}

fn install_shutdown_flag() -> CliResult<Arc<AtomicBool>> {
    let interrupted = Arc::new(AtomicBool::new(false));
    let handler_flag = Arc::clone(&interrupted);
    ctrlc::set_handler(move || {
        handler_flag.store(true, Ordering::SeqCst);
    })
    .map_err(|err| format!("failed to install SIGINT handler: {err}"))?;
    Ok(interrupted)
}

fn write_mutated_patch(
    output: &Path,
    task_id: &str,
    outcome: &PatchMutationOutcome,
) -> CliResult<(PathBuf, PathBuf)> {
    let relative_path = PathBuf::from("mutations")
        .join(task_id)
        .join(format!("{}.patch", outcome.category.slug()));
    let dir = output.join("mutations").join(task_id);
    fs::create_dir_all(&dir)?;
    let path = output.join(&relative_path);
    write_text_checked(&path, &outcome.mutated_patch)?;
    Ok((path, relative_path))
}

fn run_oracle_for_entry(
    args: &VerifyArgs,
    index: &CorpusIndex,
    entry: &GeneratedEntry,
    run_id: &str,
) -> CliResult<OracleVerdict> {
    let patch_path = resolve_corpus_path(&args.corpus, &entry.patch_path)?;
    let patch_text = fs::read_to_string(&patch_path)?;
    if sha256_text(&patch_text) != entry.patch_sha256 {
        return Err(format!(
            "cannot re-run oracle sample for {}|{} because patch hash changed on disk",
            entry.task_id,
            entry.category.slug()
        )
        .into());
    }
    let prediction = SwebenchPrediction {
        instance_id: entry.task_id.clone(),
        model_name_or_path: format!("mejepa-verify-{}", entry.category.slug()),
        model_patch: patch_text,
    };
    let mut config = SwebenchOracleConfig::defaults_for(
        entry.task_id.clone(),
        run_id.to_string(),
        args.corpus.join("swebench-verify-runs"),
        args.venv_python.clone(),
        SwebenchPredictionMode::Custom(prediction),
    );
    config.dataset_name = index.dataset_name.clone();
    config.split = index.dataset_split.clone();
    config.instance_timeout = Duration::from_secs(args.instance_timeout_secs);
    config.overall_timeout = Duration::from_secs(args.overall_timeout_secs);
    let result = run_swebench_lite_oracle(&config)?;
    Ok(result.verdict)
}

fn oracle_repeat_run_id(
    args: &VerifyArgs,
    entry: &GeneratedEntry,
    repeat_run_index: usize,
) -> CliResult<String> {
    docker_run_id(&format!(
        "{}-{}-{}-r{}-{}",
        args.run_id_prefix,
        entry.task_id,
        entry.category.slug(),
        repeat_run_index,
        unix_ms()
    ))
}

fn oracle_repeat_run_json(run_id: String, verdict: &OracleVerdict) -> CliResult<Value> {
    Ok(json!({
        "run_id": run_id,
        "oracle_all_passed": verdict.all_passed(),
        "oracle_verdict_sha256": sha256_json_value(&serde_json::to_value(verdict)?),
    }))
}

fn build_oracle_repeat_task_observation(entry: &GeneratedEntry, runs: Vec<Value>) -> Value {
    json!({
        "task_id": entry.task_id,
        "repo": entry.repo,
        "category": entry.category.slug(),
        "runs": runs,
    })
}

fn write_index(output: &Path, index: &CorpusIndex) -> CliResult<()> {
    write_json_checked(&output.join("index.json"), &serde_json::to_value(index)?)
}

fn read_index(corpus: &Path) -> CliResult<CorpusIndex> {
    Ok(serde_json::from_str(&fs::read_to_string(
        corpus.join("index.json"),
    )?)?)
}

fn validate_complete_index(index: &CorpusIndex) -> CliResult<()> {
    if !index.complete {
        return Err(
            "corpus index is incomplete; rerun generate into a fresh output directory".into(),
        );
    }
    if index.entries.is_empty() {
        return Err("corpus index has zero entries; this is not valid corpus evidence".into());
    }
    Ok(())
}

fn validate_merge_compatible(
    expected: &CorpusIndex,
    actual: &CorpusIndex,
    shard: &Path,
) -> CliResult<()> {
    if actual.schema_version != expected.schema_version {
        return Err(format!("shard {} has incompatible schema_version", shard.display()).into());
    }
    if actual.corpus_version != expected.corpus_version {
        return Err(format!("shard {} has incompatible corpus_version", shard.display()).into());
    }
    if actual.embedder_version != expected.embedder_version {
        return Err(format!(
            "shard {} has incompatible embedder_version",
            shard.display()
        )
        .into());
    }
    if actual.dataset_name != expected.dataset_name {
        return Err(format!("shard {} has incompatible dataset_name", shard.display()).into());
    }
    if actual.dataset_split != expected.dataset_split {
        return Err(format!("shard {} has incompatible dataset_split", shard.display()).into());
    }
    if actual.seed != expected.seed {
        return Err(format!("shard {} has incompatible seed", shard.display()).into());
    }
    if actual.languages != expected.languages {
        return Err(format!("shard {} has incompatible languages", shard.display()).into());
    }
    if actual.tasks_dir != expected.tasks_dir {
        return Err(format!("shard {} has incompatible tasks_dir", shard.display()).into());
    }
    if actual.split_assignments != expected.split_assignments {
        return Err(format!(
            "shard {} has incompatible split assignments",
            shard.display()
        )
        .into());
    }
    Ok(())
}

fn category_position(category: MutationCategory) -> usize {
    MutationCategory::all()
        .iter()
        .position(|candidate| *candidate == category)
        .unwrap_or(usize::MAX)
}

fn resolve_corpus_path(corpus: &Path, stored_path: &str) -> CliResult<PathBuf> {
    let path = Path::new(stored_path);
    if path.is_absolute() {
        return Err(format!(
            "corpus index path must be relative but was absolute: {stored_path}; regenerate the corpus with this CLI so verification cannot read stale external state"
        )
        .into());
    }
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return Err(format!("corpus index path escapes the corpus root: {stored_path}").into());
        }
    }
    Ok(corpus.join(path))
}

fn build_stats_json(output: &Path, index: &CorpusIndex, store: &OracleStore) -> CliResult<Value> {
    let mut by_category = BTreeMap::<String, usize>::new();
    let mut by_bucket = BTreeMap::<String, usize>::new();
    let mut by_language = BTreeMap::<String, usize>::new();
    let mut by_language_category = BTreeMap::<String, usize>::new();
    let mut by_oracle = BTreeMap::<String, usize>::new();
    for entry in &index.entries {
        *by_category
            .entry(entry.category.slug().to_string())
            .or_default() += 1;
        *by_bucket.entry(entry.bucket.clone()).or_default() += 1;
        *by_language
            .entry(entry.language.slug().to_string())
            .or_default() += 1;
        *by_language_category
            .entry(format!(
                "{}|{}",
                entry.language.slug(),
                entry.category.slug()
            ))
            .or_default() += 1;
        let key = if entry.oracle_all_passed {
            "pass"
        } else {
            "fail_or_error"
        };
        *by_oracle.entry(key.to_string()).or_default() += 1;
    }
    let expected_full_entries = index.split_assignments.len() * MutationCategory::all().len();
    let python_only = index.languages == vec![Language::Python];
    let full_ship_gate_300x8_passes = python_only
        && index.split_assignments.len() == 300
        && index.entries.len() == 300 * MutationCategory::all().len()
        && MutationCategory::all().iter().all(|category| {
            by_category
                .get(category.slug())
                .copied()
                .unwrap_or_default()
                == 300
        });
    let expected_non_python = non_python_languages()
        .iter()
        .map(|language| language.slug().to_string())
        .collect::<BTreeSet<_>>();
    let actual_languages = index
        .languages
        .iter()
        .map(|language| language.slug().to_string())
        .collect::<BTreeSet<_>>();
    let non_python_fixture_gate_passes = actual_languages == expected_non_python
        && expected_non_python.iter().all(|language| {
            by_language.get(language).copied().unwrap_or_default() >= 500
                && MutationCategory::all().iter().all(|category| {
                    by_language_category
                        .get(&format!("{language}|{}", category.slug()))
                        .copied()
                        .unwrap_or_default()
                        > 0
                })
        });
    let provenance = store.get_provenance(&index.corpus_version, &index.embedder_version)?;
    let leakage_failures = split_leakage_failures(index);
    let leakage_passes = leakage_failures.is_empty();
    let ship_gate_passes =
        leakage_passes && (full_ship_gate_300x8_passes || non_python_fixture_gate_passes);
    Ok(json!({
        "source_of_truth": {
            "index": output.join("index.json").display().to_string(),
            "store": resolve_corpus_path(output, &index.store_path)?.display().to_string(),
        },
        "provenance": provenance,
        "total_entries": index.entries.len(),
        "selected_task_count": index.selected_task_ids.len(),
        "selected_task_ids": &index.selected_task_ids,
        "task_manifest_count": index.split_assignments.len(),
        "expected_full_phase0_entries": expected_full_entries,
        "full_phase0_complete": index.entries.len() == expected_full_entries,
        "full_ship_gate_300x8": {
            "required_task_count": 300,
            "required_category_count": MutationCategory::all().len(),
            "required_entry_count": 300 * MutationCategory::all().len(),
            "actual_task_count": index.split_assignments.len(),
            "actual_entry_count": index.entries.len(),
            "passes": full_ship_gate_300x8_passes,
        },
        "by_category": by_category,
        "by_language": by_language,
        "by_language_category": by_language_category,
        "by_bucket": by_bucket,
        "by_oracle": by_oracle,
        "mutation_cf_count": store.count_cf(CF_MEJEPA_CORPUS_MUTATION_OUTCOMES)?,
        "verdict_cf_count": store.count_cf(CF_MEJEPA_CORPUS_ORACLE_VERDICTS_JSON)?,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE_JSON)?,
        "leakage_check": {
            "failures": leakage_failures,
            "passes": leakage_passes,
        },
        "non_python_fixture_gate": {
            "required_languages": expected_non_python,
            "actual_languages": actual_languages,
            "min_entries_per_language": 500,
            "passes": non_python_fixture_gate_passes,
        },
        "passes": ship_gate_passes,
    }))
}

fn write_stats_json(output: &Path, stats: &Value) -> CliResult<()> {
    write_json_checked(&output.join("stats.json"), stats)
}

fn stats_source_of_truth_passes(stats: &Value, expected_entries: usize) -> bool {
    let expected = expected_entries as u64;
    stats.get("total_entries").and_then(Value::as_u64) == Some(expected)
        && stats.get("mutation_cf_count").and_then(Value::as_u64) == Some(expected)
        && stats.get("verdict_cf_count").and_then(Value::as_u64) == Some(expected)
        && stats.get("provenance_cf_count").and_then(Value::as_u64) == Some(1)
        && stats
            .get("leakage_check")
            .and_then(|value| value.get("passes"))
            .and_then(Value::as_bool)
            == Some(true)
        && stats
            .get("provenance")
            .and_then(|value| value.get("complete"))
            .and_then(Value::as_bool)
            == Some(true)
        && stats
            .get("provenance")
            .and_then(|value| value.get("mutation_count"))
            .and_then(Value::as_u64)
            == Some(expected)
}

fn build_provenance(index: &CorpusIndex) -> CliResult<CorpusProvenance> {
    let rows = index
        .entries
        .iter()
        .map(|entry| CorpusProvenanceRow {
            task_id: entry.task_id.clone(),
            category: entry.category,
            patch_sha256: entry.patch_sha256.clone(),
            oracle_verdict_sha256: entry.oracle_verdict_sha256.clone(),
        })
        .collect::<Vec<_>>();
    let corpus_sha256 = compute_corpus_sha256(&rows).map_err(|err| {
        format!(
            "failed to compute corpus provenance hash: {err} ({})",
            err.code()
        )
    })?;
    let mut categories = index
        .entries
        .iter()
        .map(|entry| entry.category)
        .collect::<Vec<_>>();
    categories.sort_by_key(|category| category.slug());
    categories.dedup();
    let languages = if index.languages.is_empty() {
        vec![Language::Python]
    } else {
        index.languages.clone()
    };
    let provenance = CorpusProvenance {
        corpus_version: index.corpus_version.clone(),
        embedder_version: index.embedder_version.clone(),
        generated_at_unix_ms: index.generated_at_unix_ms,
        generator_version: env!("CARGO_PKG_VERSION").to_string(),
        seed: index.seed,
        languages,
        task_manifest_count: index.split_assignments.len(),
        mutation_count: index.entries.len(),
        mutation_categories: categories,
        source_patch_mode: "source-backed".to_string(),
        split_mode: "instance_atomic".to_string(),
        corpus_sha256,
        complete: true,
    };
    provenance
        .validate()
        .map_err(|err| format!("invalid corpus provenance: {err} ({})", err.code()))?;
    Ok(provenance)
}

fn deterministic_sample<'a>(
    entries: &'a [GeneratedEntry],
    sample_fraction: f64,
    corpus_sha256: &str,
) -> CliResult<Vec<&'a GeneratedEntry>> {
    if entries.is_empty() {
        return Err("cannot sample zero corpus entries".into());
    }
    let count = ((entries.len() as f64) * sample_fraction).ceil() as usize;
    let count = count.clamp(1, entries.len());
    let mut keyed = entries
        .iter()
        .map(|entry| {
            let digest = Sha256::digest(
                format!(
                    "{}|{}|{}",
                    corpus_sha256,
                    entry.task_id,
                    entry.category.slug()
                )
                .as_bytes(),
            );
            (digest.to_vec(), entry)
        })
        .collect::<Vec<_>>();
    keyed.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(keyed
        .into_iter()
        .take(count)
        .map(|(_, entry)| entry)
        .collect())
}

fn select_oracle_samples<'a>(
    entries: &'a [GeneratedEntry],
    sample_fraction: f64,
    corpus_sha256: &str,
    sample_keys: &[String],
) -> CliResult<Vec<&'a GeneratedEntry>> {
    let mut selected = deterministic_sample(entries, sample_fraction, corpus_sha256)?;
    let mut seen = selected
        .iter()
        .map(|entry| corpus_row_key(&entry.task_id, entry.category))
        .collect::<BTreeSet<_>>();
    for sample_key in sample_keys {
        let (task_id, category) = parse_sample_key(sample_key)?;
        let entry = entries
            .iter()
            .find(|entry| entry.task_id == task_id && entry.category == category)
            .ok_or_else(|| {
                format!(
                    "sample-key `{sample_key}` does not exist in corpus index; expected task_id|category"
                )
            })?;
        let key = corpus_row_key(&entry.task_id, entry.category);
        if seen.insert(key) {
            selected.push(entry);
        }
    }
    Ok(selected)
}

fn parse_sample_key(value: &str) -> CliResult<(&str, MutationCategory)> {
    let (task_id, category_slug) = value
        .split_once('|')
        .ok_or_else(|| format!("sample-key `{value}` must have format task_id|category"))?;
    if task_id.trim().is_empty() || category_slug.trim().is_empty() {
        return Err(
            format!("sample-key `{value}` must have non-empty task_id and category").into(),
        );
    }
    let category = parse_category(category_slug)?;
    Ok((task_id, category))
}

fn split_leakage_failures(index: &CorpusIndex) -> Vec<Value> {
    let mut bucket_by_task = BTreeMap::<&str, &str>::new();
    let mut failures = Vec::new();
    for assignment in &index.split_assignments {
        if let Some(previous) =
            bucket_by_task.insert(assignment.task_id.as_str(), assignment.bucket.slug())
        {
            if previous != assignment.bucket.slug() {
                failures.push(json!({
                    "task_id": assignment.task_id.clone(),
                    "first_bucket": previous,
                    "second_bucket": assignment.bucket.slug(),
                }));
            }
        }
    }
    failures
}

fn default_corpus_version() -> String {
    MEJEPA_CORPUS_VERSION_V1.to_string()
}

fn default_embedder_version() -> String {
    MEJEPA_EMBEDDER_VERSION_V1.to_string()
}

fn write_json_checked(path: &Path, value: &Value) -> CliResult<()> {
    write_text_checked(path, &serde_json::to_string_pretty(value)?)
}

fn read_optional_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn write_text_checked(path: &Path, text: &str) -> CliResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "write_text_checked: create_dir_all parent={} failed: {err}; state={}",
                parent.display(),
                describe_path_state(parent)
            )
        })?;
    }
    let parent = path
        .parent()
        .ok_or_else(|| format!("cannot write path without parent: {}", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            format!(
                "cannot write path with non-UTF8 file name: {}",
                path.display()
            )
        })?;
    // Use a process/thread/counter-qualified temp name. Even if a future call
    // site writes concurrently in the same parent, temp-file collisions fail
    // closed at create_new() instead of silently truncating another writer.
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
    let thread_id = format!("{:?}", std::thread::current().id())
        .replace("ThreadId(", "")
        .replace(')', "");
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-t{}-{}-{}",
        file_name,
        std::process::id(),
        thread_id,
        unix_ms(),
        counter,
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|err| {
        format!(
            "write_text_checked: open(create_new) temp_path={} parent={} failed: {err}; temp_state={}; parent_state={}",
            temp_path.display(),
            parent.display(),
            describe_path_state(&temp_path),
            describe_path_state(parent)
        )
    })?;
    file.write_all(text.as_bytes()).map_err(|err| {
        format!(
            "write_text_checked: write_all to {} failed: {err}",
            temp_path.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "write_text_checked: sync_all on {} failed: {err}",
            temp_path.display()
        )
    })?;
    drop(file);
    fs::rename(&temp_path, path).map_err(|err| {
        format!(
            "write_text_checked: rename {} -> {} failed (temp_exists={}, target_parent_exists={}): {err}",
            temp_path.display(),
            path.display(),
            temp_path.exists(),
            path.parent().is_some_and(|p| p.exists())
        )
    })?;
    if let Some(parent) = path.parent() {
        File::open(parent)
            .map_err(|err| {
                format!(
                    "write_text_checked: File::open parent {} for sync failed: {err}",
                    parent.display()
                )
            })?
            .sync_all()
            .map_err(|err| {
                format!(
                    "write_text_checked: parent sync_all {} failed: {err}",
                    parent.display()
                )
            })?;
    }
    let readback = fs::read_to_string(path).map_err(|err| {
        format!(
            "write_text_checked: readback fs::read_to_string {} failed immediately after rename+fsync: {err}; target_state={}; parent_state={}",
            path.display(),
            describe_path_state(path),
            describe_path_state(parent)
        )
    })?;
    if readback != text {
        return Err(format!(
            "write_text_checked: readback mismatch after writing {}; expected_len={} actual_len={} expected_sha256={} actual_sha256={}",
            path.display(),
            text.len(),
            readback.len(),
            sha256_text(text),
            sha256_text(&readback)
        )
        .into());
    }
    Ok(())
}

fn describe_path_state(path: &Path) -> String {
    match fs::metadata(path) {
        Ok(metadata) => format!(
            "exists=true is_dir={} is_file={} len={} readonly={}",
            metadata.is_dir(),
            metadata.is_file(),
            metadata.len(),
            metadata.permissions().readonly()
        ),
        Err(err) => format!("exists=false metadata_error={err}"),
    }
}

fn duration_secs(duration: Duration) -> f64 {
    duration.as_secs_f64()
}

fn stable_seed(global_seed: u64, task_id: &str, category: MutationCategory) -> u64 {
    let digest =
        Sha256::digest(format!("{}|{}|{}", global_seed, task_id, category.slug()).as_bytes());
    u64::from_le_bytes(digest[0..8].try_into().expect("slice has 8 bytes"))
}

fn corpus_row_key(task_id: &str, category: MutationCategory) -> String {
    format!("{}|{}", task_id, category.slug())
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn sha256_path(path: &Path) -> CliResult<String> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{:x}", Sha256::digest(&bytes)))
}

fn sha256_json_value(value: &Value) -> String {
    sha256_text(&serde_json::to_string(value).expect("JSON value serialises"))
}

fn docker_run_id(raw: &str) -> CliResult<String> {
    validate_docker_run_id_component(raw)?;
    Ok(raw.to_string())
}

fn validate_docker_run_id_component(raw: &str) -> CliResult<()> {
    let mut chars = raw.chars();
    let first = chars
        .next()
        .ok_or("Docker run id component must be non-empty")?;
    if !first.is_ascii_alphanumeric() {
        return Err(format!(
            "Docker run id component `{raw}` must start with an ASCII alphanumeric character"
        )
        .into());
    }
    if std::iter::once(first)
        .chain(chars)
        .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-'))
    {
        return Err(format!(
            "Docker run id component `{raw}` contains invalid characters; allowed alphabet is [A-Za-z0-9_.-]"
        )
        .into());
    }
    Ok(())
}

fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_mejepa_corpus::split::SplitBucket;

    #[test]
    fn swebench_oracle_cli_defaults_to_aiwonder_hot_python() -> CliResult<()> {
        let expected = PathBuf::from(AIWONDER_SWEBENCH_PYTHON);

        let generate = Cli::try_parse_from([
            "mejepa-corpus",
            "generate",
            "--output",
            "/zfs/archive/contextgraph/corpus/synthetic-default-python",
        ])?;
        match generate.command {
            CommandKind::Generate(args) => assert_eq!(args.venv_python, expected),
            _ => panic!("expected generate command"),
        }

        let verify = Cli::try_parse_from([
            "mejepa-corpus",
            "verify",
            "--corpus",
            "/zfs/archive/contextgraph/corpus/synthetic-default-python",
        ])?;
        match verify.command {
            CommandKind::Verify(args) => assert_eq!(args.venv_python, expected),
            _ => panic!("expected verify command"),
        }

        let smoke = Cli::try_parse_from([
            "mejepa-corpus",
            "smoke",
            "--output",
            "/zfs/archive/contextgraph/corpus/synthetic-default-python-smoke",
        ])?;
        match smoke.command {
            CommandKind::Smoke(args) => assert_eq!(args.venv_python, expected),
            _ => panic!("expected smoke command"),
        }

        Ok(())
    }

    #[test]
    fn smoke_stats_integrity_allows_partial_corpus_without_full_ship_gate() {
        let stats = json!({
            "total_entries": 8,
            "mutation_cf_count": 8,
            "verdict_cf_count": 8,
            "provenance_cf_count": 1,
            "leakage_check": {"passes": true},
            "provenance": {"complete": true, "mutation_count": 8},
            "passes": false,
            "full_ship_gate_300x8": {"passes": false},
        });
        assert!(stats_source_of_truth_passes(&stats, 8));

        let mut tampered = stats.clone();
        tampered["verdict_cf_count"] = json!(7);
        assert!(!stats_source_of_truth_passes(&tampered, 8));
    }

    #[test]
    fn oracle_repeat_output_matches_flakiness_audit_schema() -> CliResult<()> {
        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_synthetic.py::test_passes".to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 7,
            }],
            exception: None,
            evidence_unavailable: false,
        };
        let entry = GeneratedEntry {
            task_id: "synthetic_task_repeat_schema".to_string(),
            repo: "synthetic/repo".to_string(),
            language: Language::Python,
            category: MutationCategory::KnownGood,
            bucket: "train".to_string(),
            patch_path: "mutations/synthetic_task_repeat_schema/known_good.patch".to_string(),
            mutation_note: "synthetic".to_string(),
            patch_sha256: sha256_text("patch"),
            oracle_verdict_sha256: sha256_json_value(&serde_json::to_value(&verdict)?),
            oracle_all_passed: true,
            oracle_exception: None,
            oracle_per_test_count: 1,
        };

        let run = oracle_repeat_run_json("repeat-run-1".to_string(), &verdict)?;
        let task = build_oracle_repeat_task_observation(&entry, vec![run.clone()]);
        let observations = json!({ "tasks": [task] });

        assert_eq!(observations["tasks"][0]["task_id"], entry.task_id);
        assert_eq!(observations["tasks"][0]["repo"], entry.repo);
        assert_eq!(observations["tasks"][0]["category"], entry.category.slug());
        assert_eq!(observations["tasks"][0]["runs"][0], run);
        assert!(observations.as_object().is_some_and(|map| map.len() == 1));
        assert!(run.as_object().is_some_and(|map| map.len() == 3));
        Ok(())
    }

    #[test]
    fn oracle_repeat_output_requires_multiple_runs() -> CliResult<()> {
        let venv = tempfile::NamedTempFile::new()?;
        let args = VerifyArgs {
            corpus: PathBuf::from("/zfs/archive/contextgraph/corpus/synthetic-repeat-check"),
            sample_fraction: 1.0,
            sample_keys: Vec::new(),
            quarantine_config: None,
            venv_python: venv.path().to_path_buf(),
            run_id_prefix: "synthetic-repeat-check".to_string(),
            instance_timeout_secs: 1,
            overall_timeout_secs: 1,
            oracle_repeat_runs: 1,
            oracle_repeat_output: Some(PathBuf::from(
                "/zfs/archive/contextgraph/exports/eval/synthetic-repeat-check.json",
            )),
        };

        let error = validate_verify_args(&args).expect_err("single-run output must fail closed");
        assert!(error
            .to_string()
            .contains("oracle_repeat_runs must be >= 2"));
        Ok(())
    }

    #[test]
    fn corpus_quarantine_loader_accepts_flakiness_audit_toml() -> CliResult<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("corpus_quarantine.toml");
        fs::write(
            &path,
            r#"
[[quarantined_tasks]]
task_id = "psf__requests-1766"
reason = "oracle flakiness 0.500000 > threshold 0.050000 over 2 runs for psf__requests-1766|known_good"
flakiness_rate = 0.5
oracle_runs = 2
observed_outcomes = [
    false,
    true,
]
observed_verdict_sha256 = [
    "0adf3e5745f5e0e974f4c3f3575dd1f8ed85203128a88e77a6fefe56311dc9b0",
    "sha256:a1e56fa06f67cc74766c75cc54a4de27f8a02958a787dfa1ef142e47a087de26",
]
operator_id = "test"
created_unix_ms = 1
"#,
        )?;

        let quarantine = load_corpus_quarantine(&path)?;

        assert_eq!(quarantine.len(), 1);
        let entry = quarantine.get("psf__requests-1766").unwrap();
        assert_eq!(entry.oracle_runs, 2);
        assert_eq!(entry.observed_outcomes, vec![false, true]);
        assert_eq!(entry.flakiness_rate, 0.5);
        Ok(())
    }

    #[test]
    fn corpus_quarantine_loader_rejects_duplicate_tasks() -> CliResult<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("corpus_quarantine.toml");
        fs::write(
            &path,
            r#"
[[quarantined_tasks]]
task_id = "duplicate-task"
reason = "first"
flakiness_rate = 0.5
oracle_runs = 2
observed_outcomes = [false, true]
observed_verdict_sha256 = [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
]
operator_id = "test"
created_unix_ms = 1

[[quarantined_tasks]]
task_id = "duplicate-task"
reason = "second"
flakiness_rate = 0.5
oracle_runs = 2
observed_outcomes = [false, true]
observed_verdict_sha256 = [
    "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
]
operator_id = "test"
created_unix_ms = 1
"#,
        )?;

        let err = load_corpus_quarantine(&path).expect_err("duplicate quarantine must fail");

        assert!(err
            .to_string()
            .contains("MEJEPA_CORPUS_QUARANTINE_DUPLICATE_TASK"));
        Ok(())
    }

    #[test]
    fn prune_quarantine_excludes_task_from_index_and_store() -> CliResult<()> {
        let dir = tempfile::tempdir()?;
        let source = dir.path().join("source-corpus");
        let output = dir.path().join("pruned-corpus");
        fs::create_dir_all(&source)?;

        let verdict = OracleVerdict {
            per_test: vec![PerTestOutcome {
                test_id: "tests/test_synthetic.py::test_outcome".to_string(),
                outcome: TestOutcome::Pass,
                runtime_ms: 1,
            }],
            exception: None,
            evidence_unavailable: false,
        };
        let store_path_rel = PathBuf::from("oracle-store");
        let store_path = source.join(&store_path_rel);
        let store = OracleStore::open(&store_path)?;
        let mut entries = Vec::new();
        for task_id in ["keep_task", "drop_task"] {
            let patch_text = format!("patch for {task_id}\n");
            let patch_path = PathBuf::from("mutations")
                .join(task_id)
                .join("known_good.patch");
            write_text_checked(&source.join(&patch_path), &patch_text)?;
            let mutation = MutationOutcome {
                category: MutationCategory::KnownGood,
                mutated_source: patch_text.clone(),
                seed: 7,
                mutation_site: None,
            };
            store.put_corpus_row(task_id, MutationCategory::KnownGood, &mutation, &verdict)?;
            entries.push(GeneratedEntry {
                task_id: task_id.to_string(),
                repo: "synthetic/repo".to_string(),
                language: Language::Python,
                category: MutationCategory::KnownGood,
                bucket: "train".to_string(),
                patch_path: patch_path.display().to_string(),
                mutation_note: "synthetic".to_string(),
                patch_sha256: sha256_text(&patch_text),
                oracle_verdict_sha256: sha256_json_value(&serde_json::to_value(&verdict)?),
                oracle_all_passed: true,
                oracle_exception: None,
                oracle_per_test_count: 1,
            });
        }
        store.flush()?;
        drop(store);

        let index = CorpusIndex {
            schema_version: 1,
            corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
            embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
            dataset_name: "synthetic/prune-quarantine".to_string(),
            dataset_split: "test".to_string(),
            complete: true,
            generated_at_unix_ms: 1,
            seed: 0,
            languages: vec![Language::Python],
            tasks_dir: "synthetic".to_string(),
            store_path: store_path_rel.display().to_string(),
            corpus_sha256: None,
            selected_task_ids: vec!["keep_task".to_string(), "drop_task".to_string()],
            entries,
            split_assignments: vec![
                TaskAssignment {
                    task_id: "keep_task".to_string(),
                    repo: "synthetic/repo".to_string(),
                    bucket: SplitBucket::Train,
                },
                TaskAssignment {
                    task_id: "drop_task".to_string(),
                    repo: "synthetic/repo".to_string(),
                    bucket: SplitBucket::Train,
                },
            ],
        };
        write_index(&source, &index)?;
        let quarantine_path = dir.path().join("corpus_quarantine.toml");
        fs::write(
            &quarantine_path,
            r#"
[[quarantined_tasks]]
task_id = "drop_task"
reason = "synthetic flake"
flakiness_rate = 0.5
oracle_runs = 2
observed_outcomes = [false, true]
observed_verdict_sha256 = [
    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
]
operator_id = "test"
created_unix_ms = 1
"#,
        )?;

        prune_quarantine_impl(PruneQuarantineArgs {
            corpus: source.clone(),
            quarantine_config: quarantine_path,
            output: output.clone(),
        })?;

        let pruned_index = read_index(&output)?;
        assert!(pruned_index.complete);
        assert_eq!(pruned_index.entries.len(), 1);
        assert_eq!(
            pruned_index.selected_task_ids,
            vec!["keep_task".to_string()]
        );
        assert_eq!(pruned_index.entries[0].task_id, "keep_task");
        let pruned_store = OracleStore::open(output.join("oracle-store"))?;
        assert!(pruned_store
            .get_corpus_row("keep_task", MutationCategory::KnownGood)?
            .is_some());
        assert!(pruned_store
            .get_corpus_row("drop_task", MutationCategory::KnownGood)?
            .is_none());
        let evidence: Value = serde_json::from_str(&fs::read_to_string(
            output.join("prune-quarantine-evidence.json"),
        )?)?;
        assert_eq!(evidence["entries"], json!(1));
        assert_eq!(evidence["excluded_entry_count"], json!(1));
        assert_eq!(evidence["passes"], json!(true));
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn wait_for_child_status_handles_large_file_backed_output() -> CliResult<()> {
        let dir = tempfile::tempdir()?;
        let stdout_path = dir.path().join("large-child.stdout.log");
        let stderr_path = dir.path().join("large-child.stderr.log");

        println!(
            "FSV_BEFORE large_output stdout_exists={} stderr_exists={}",
            stdout_path.exists(),
            stderr_path.exists()
        );

        let stdout_file = File::create(&stdout_path)?;
        let stderr_file = File::create(&stderr_path)?;
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(
                "i=0; while [ \"$i\" -lt 20000 ]; do \
                 printf 'stdout-%05d xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\\n' \"$i\"; \
                 printf 'stderr-%05d yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy\\n' \"$i\" >&2; \
                 i=$((i+1)); \
                 done",
            )
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file));
        configure_child_process_group(&mut command);

        let status = wait_for_child_status(
            command.spawn()?,
            Duration::from_secs(10),
            "large-output-fsv",
        )?;
        assert!(status.success());

        let stdout_len = fs::metadata(&stdout_path)?.len();
        let stderr_len = fs::metadata(&stderr_path)?.len();
        let stdout_tail = tail_file(&stdout_path)?;
        let stderr_tail = tail_file(&stderr_path)?;

        println!(
            "FSV_AFTER large_output stdout_path={} stdout_bytes={} stdout_tail_has_last={} stderr_path={} stderr_bytes={} stderr_tail_has_last={}",
            stdout_path.display(),
            stdout_len,
            stdout_tail.contains("stdout-19999"),
            stderr_path.display(),
            stderr_len,
            stderr_tail.contains("stderr-19999")
        );

        assert!(stdout_len > LOG_TAIL_BYTES);
        assert!(stderr_len > LOG_TAIL_BYTES);
        assert!(stdout_tail.contains("stdout-19999"));
        assert!(stderr_tail.contains("stderr-19999"));
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn wait_for_child_status_times_out_and_reaps_child() -> CliResult<()> {
        let dir = tempfile::tempdir()?;
        let pid_path = dir.path().join("sleeping-child.pid");

        println!(
            "FSV_BEFORE timeout pid_file_exists={} pid_file={}",
            pid_path.exists(),
            pid_path.display()
        );

        let script = format!("printf '%s' $$ > {}; sleep 30", pid_path.display());
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg(script)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_child_process_group(&mut command);
        let child = command.spawn()?;

        let pid_ready_started = Instant::now();
        while !pid_path.exists() {
            if pid_ready_started.elapsed() > Duration::from_secs(2) {
                return Err(format!("child did not write pid file {}", pid_path.display()).into());
            }
            thread::sleep(Duration::from_millis(10));
        }
        let pid = fs::read_to_string(&pid_path)?.trim().parse::<u32>()?;
        let proc_path = PathBuf::from(format!("/proc/{pid}"));

        let error = wait_for_child_status(child, Duration::from_millis(50), "timeout-fsv")
            .expect_err("sleeping child must time out");
        let proc_wait_started = Instant::now();
        while proc_path.exists() && proc_wait_started.elapsed() < Duration::from_secs(2) {
            thread::sleep(Duration::from_millis(10));
        }

        println!(
            "FSV_AFTER timeout pid={} proc_path={} proc_exists={} error={}",
            pid,
            proc_path.display(),
            proc_path.exists(),
            error
        );

        assert!(error.to_string().contains("timed out"));
        assert!(!proc_path.exists());
        Ok(())
    }

    #[test]
    fn pull_log_paths_are_deterministic_and_content_addressed() -> CliResult<()> {
        let image = "swebench/sweb.eval.x86_64.sympy_1776_sympy-20590:latest";
        let before = pull_log_paths(image)?;
        let after = pull_log_paths(image)?;
        let log_root = context_graph_paths::mejepa_image_prep_log_dir()?;

        println!(
            "FSV_BEFORE log_paths stdout={} stderr={}",
            before.stdout.display(),
            before.stderr.display()
        );
        println!(
            "FSV_AFTER log_paths same_stdout={} same_stderr={} parent_exists={}",
            before.stdout == after.stdout,
            before.stderr == after.stderr,
            before.stdout.parent().is_some_and(Path::exists)
        );

        assert_eq!(before.stdout, after.stdout);
        assert_eq!(before.stderr, after.stderr);
        assert!(before.stdout.starts_with(&log_root));
        assert!(before.stderr.starts_with(&log_root));
        assert_ne!(before.stdout, before.stderr);
        let stdout_name = before
            .stdout
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("stdout log path has no valid file name")?;
        assert!(stdout_name.starts_with("docker-pull-"));
        assert!(!stdout_name.contains(':'));
        Ok(())
    }
}
