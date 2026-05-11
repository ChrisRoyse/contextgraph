use clap::{Parser, Subcommand};
use context_graph_mejepa_corpus::oracle::{
    OracleStore, CF_MEJEPA_CORPUS_PROVENANCE, CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
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
use context_graph_mejepa_corpus::{
    compute_corpus_sha256, parse_languages, CorpusProvenance, CorpusProvenanceRow, Language,
    MutationCategory, MutationOutcome, MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
};
use context_graph_mejepa_instruments::OracleVerdict;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
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
    PrepareImages(PrepareImagesArgs),
    Cleanup(CleanupArgs),
}

#[derive(Parser, Debug, Clone)]
struct GenerateArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python"
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
    #[arg(long, default_value = "/mnt/d/contextgraph/cache/mejepa-corpus/repos")]
    repo_cache_dir: PathBuf,
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/work/mejepa-corpus/source-work"
    )]
    source_work_root: PathBuf,
    #[arg(long, default_value = "mejepa-corpus")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
    #[arg(long, default_value_t = 4)]
    oracle_workers: usize,
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
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python"
    )]
    venv_python: PathBuf,
    #[arg(long, default_value = "mejepa-corpus-verify")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
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
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/cache/contextgraph-swebench/venv/bin/python"
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
    #[arg(long, default_value = "/mnt/d/contextgraph/cache/mejepa-corpus/repos")]
    repo_cache_dir: PathBuf,
    #[arg(
        long,
        default_value = "/mnt/d/contextgraph/work/mejepa-corpus/source-work"
    )]
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

fn main() -> CliResult<()> {
    match Cli::parse().command {
        CommandKind::Generate(args) => generate(args),
        CommandKind::Verify(args) => verify(args),
        CommandKind::Stats(args) => stats(args),
        CommandKind::Smoke(args) => smoke(args),
        CommandKind::Merge(args) => merge(args),
        CommandKind::Normalize(args) => normalize(args),
        CommandKind::PrepareImages(args) => prepare_images(args),
        CommandKind::Cleanup(args) => cleanup(args),
    }
}

fn generate(mut args: GenerateArgs) -> CliResult<()> {
    args.output = require_durable_path(&args.output, "generate.output")?;
    args.venv_python = require_durable_path(&args.venv_python, "generate.venv_python")?;
    args.repo_cache_dir = require_durable_path(&args.repo_cache_dir, "generate.repo_cache_dir")?;
    args.source_work_root =
        require_durable_path(&args.source_work_root, "generate.source_work_root")?;
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

    for category in &categories {
        let mut pending = Vec::with_capacity(selected.len());
        let mut predictions = Vec::with_capacity(selected.len());
        for manifest in &selected {
            let row_key = corpus_row_key(&manifest.instance_id, *category);
            if completed_keys.contains(&row_key) {
                continue;
            }
            if interrupted.load(Ordering::SeqCst) {
                write_index(&args.output, &index)?;
                return Err(
                    "generation interrupted by SIGINT before starting next corpus row; index remains complete=false"
                        .into(),
                );
            }
            let official_patch = patch_by_task.get(&manifest.instance_id).ok_or_else(|| {
                format!(
                    "official dataset patch missing after loader for {}",
                    manifest.instance_id
                )
            })?;
            let seed = stable_seed(args.seed, &manifest.instance_id, *category);
            let patch_outcome = mutate_task_source_patch(
                &SourcePatchTask {
                    instance_id: manifest.instance_id.clone(),
                    repo: manifest.repo.clone(),
                    base_commit: manifest.base_commit.clone(),
                },
                *category,
                official_patch,
                seed,
                &source_patch_config,
            )
            .map_err(|err| {
                format!(
                    "failed to build source-backed patch for task={} category={}: {} ({})",
                    manifest.instance_id,
                    category.slug(),
                    err,
                    err.code()
                )
            })?;
            let (_patch_path, patch_path_rel) =
                write_mutated_patch(&args.output, &manifest.instance_id, &patch_outcome)?;
            let patch_sha256 = sha256_text(&patch_outcome.mutated_patch);
            let mutation = MutationOutcome {
                category: *category,
                mutated_source: patch_outcome.mutated_patch.clone(),
                seed,
                mutation_site: None,
            };
            predictions.push(SwebenchPrediction {
                instance_id: manifest.instance_id.clone(),
                model_name_or_path: format!("mejepa-{}", category.slug()),
                model_patch: patch_outcome.mutated_patch.clone(),
            });
            pending.push((
                manifest.clone(),
                patch_outcome,
                patch_path_rel,
                patch_sha256,
                mutation,
            ));
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
        batch_config.instance_timeout = Duration::from_secs(args.instance_timeout_secs);
        batch_config.overall_timeout = Duration::from_secs(args.overall_timeout_secs);
        batch_config.max_workers = args.oracle_workers;
        batch_config.interrupt_flag = Some(Arc::clone(&interrupted));
        let batch_result = run_swebench_lite_oracle_batch(&batch_config)?;

        for (manifest, patch_outcome, patch_path_rel, patch_sha256, mutation) in pending {
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
    args.corpus = require_durable_path(&args.corpus, "verify.corpus")?;
    args.venv_python = require_durable_path(&args.venv_python, "verify.venv_python")?;
    validate_verify_args(&args)?;
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
    let mutation_cf_count = store.count_cf(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?;
    let provenance_cf_count = store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE)?;
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
            let fresh_verdict = run_oracle_for_entry(&args, entry)?;
            let fresh_hash = sha256_json_value(&serde_json::to_value(&fresh_verdict)?);
            let matches_persisted = fresh_hash == entry.oracle_verdict_sha256;
            if !matches_persisted {
                failures.push(json!({
                    "key": format!("{}|{}", entry.task_id, entry.category.slug()),
                    "error": "fresh oracle sample hash mismatch",
                    "expected": entry.oracle_verdict_sha256,
                    "actual": fresh_hash,
                }));
            }
            oracle_sample.push(json!({
                "task_id": entry.task_id,
                "category": entry.category.slug(),
                "persisted_oracle_verdict_sha256": entry.oracle_verdict_sha256,
                "fresh_oracle_verdict_sha256": fresh_hash,
                "persisted_all_passed": entry.oracle_all_passed,
                "fresh_all_passed": fresh_verdict.all_passed(),
                "matches_persisted": matches_persisted,
            }));
        }
    }
    let passes = failures.is_empty();
    let evidence = json!({
        "source_of_truth": {
            "index": args.corpus.join("index.json").display().to_string(),
            "store": store_path.display().to_string(),
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE,
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
    args.corpus = require_durable_path(&args.corpus, "stats.corpus")?;
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
    args.corpus = require_durable_path(&args.corpus, "cleanup.corpus")?;
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
    args.output = require_durable_path(&args.output, "merge.output")?;
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
    args.corpus = require_durable_path(&args.corpus, "normalize.corpus")?;
    args.output = require_durable_path(&args.output, "normalize.output")?;
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

    let mutation_cf_count = store.count_cf(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?;
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
            "provenance_cf": CF_MEJEPA_CORPUS_PROVENANCE,
        },
        "source_corpus_sha256": source_index.corpus_sha256,
        "normalized_corpus_sha256": provenance.corpus_sha256,
        "entries": index.entries.len(),
        "changed_entries": changed_entries,
        "changed_examples": changed_examples,
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE)?,
        "passes": true,
    });
    write_json_checked(&args.output.join("normalize-evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn smoke(mut args: SmokeArgs) -> CliResult<()> {
    args.output = require_durable_path(&args.output, "smoke.output")?;
    args.venv_python = require_durable_path(&args.venv_python, "smoke.venv_python")?;
    args.repo_cache_dir = require_durable_path(&args.repo_cache_dir, "smoke.repo_cache_dir")?;
    args.source_work_root = require_durable_path(&args.source_work_root, "smoke.source_work_root")?;
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
        resume_incomplete: args.resume_incomplete,
    };
    let verify_args = VerifyArgs {
        corpus: args.output.clone(),
        sample_fraction: args.sample_fraction,
        sample_keys: Vec::new(),
        venv_python: args.venv_python.clone(),
        run_id_prefix: format!("{}-verify", args.run_id_prefix),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
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
                if stats.get("passes").and_then(Value::as_bool) != Some(true) {
                    failures.push(json!({
                        "phase": "stats",
                        "error": "stats leakage/source-of-truth check did not pass",
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
            "reason": "SWE-bench Lite and the Phase 0 mutation/oracle path are Python-only; non-Python language enum variants fail closed until their operators and fixtures exist."
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
        resume_incomplete: args.resume_incomplete,
    })?;
    validate_verify_args(&VerifyArgs {
        corpus: args.output.clone(),
        sample_fraction: args.sample_fraction,
        sample_keys: Vec::new(),
        venv_python: args.venv_python.clone(),
        run_id_prefix: format!("{}-verify", args.run_id_prefix),
        instance_timeout_secs: args.instance_timeout_secs,
        overall_timeout_secs: args.overall_timeout_secs,
    })?;
    Ok(())
}

fn require_durable_path(path: &Path, field: &'static str) -> CliResult<PathBuf> {
    context_graph_paths::require_under_data_root(path, field).map_err(|err| err.to_string().into())
}

fn validate_generate_args(args: &GenerateArgs) -> CliResult<()> {
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
    let output = Command::new("docker").args(args).output()?;
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
    let output = Command::new("docker").args(args).output()?;
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
    Ok(())
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
    Ok(keys)
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
    instance_ids: &[&str],
) -> CliResult<BTreeMap<String, String>> {
    let ids_json = serde_json::to_string(instance_ids)?;
    let snippet = r#"
import json
import sys
from datasets import load_dataset
ids = set(json.loads(sys.argv[1]))
rows = {}
for row in load_dataset("princeton-nlp/SWE-bench_Lite", split="test"):
    if row["instance_id"] in ids:
        rows[row["instance_id"]] = row.get("patch") or ""
missing = sorted(ids - set(rows))
if missing:
    raise SystemExit("missing official dataset rows: " + ",".join(missing))
print(json.dumps(rows, sort_keys=True))
"#;
    let output = Command::new(venv_python)
        .arg("-c")
        .arg(snippet)
        .arg(ids_json)
        .output()?;
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

fn run_oracle_for_entry(args: &VerifyArgs, entry: &GeneratedEntry) -> CliResult<OracleVerdict> {
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
    let run_id = docker_run_id(&format!(
        "{}-{}-{}-{}",
        args.run_id_prefix,
        entry.task_id,
        entry.category.slug(),
        unix_ms()
    ))?;
    let mut config = SwebenchOracleConfig::defaults_for(
        entry.task_id.clone(),
        run_id,
        args.corpus.join("swebench-verify-runs"),
        args.venv_python.clone(),
        SwebenchPredictionMode::Custom(prediction),
    );
    config.instance_timeout = Duration::from_secs(args.instance_timeout_secs);
    config.overall_timeout = Duration::from_secs(args.overall_timeout_secs);
    let result = run_swebench_lite_oracle(&config)?;
    Ok(result.verdict)
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
    let mut by_oracle = BTreeMap::<String, usize>::new();
    for entry in &index.entries {
        *by_category
            .entry(entry.category.slug().to_string())
            .or_default() += 1;
        *by_bucket.entry(entry.bucket.clone()).or_default() += 1;
        let key = if entry.oracle_all_passed {
            "pass"
        } else {
            "fail_or_error"
        };
        *by_oracle.entry(key.to_string()).or_default() += 1;
    }
    let expected_full_entries = index.split_assignments.len() * MutationCategory::all().len();
    let full_ship_gate_300x8_passes = index.split_assignments.len() == 300
        && index.entries.len() == 300 * MutationCategory::all().len()
        && MutationCategory::all().iter().all(|category| {
            by_category
                .get(category.slug())
                .copied()
                .unwrap_or_default()
                == 300
        });
    let provenance = store.get_provenance(&index.corpus_version, &index.embedder_version)?;
    let leakage_failures = split_leakage_failures(index);
    let leakage_passes = leakage_failures.is_empty();
    let ship_gate_passes = leakage_passes && full_ship_gate_300x8_passes;
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
        "by_bucket": by_bucket,
        "by_oracle": by_oracle,
        "mutation_cf_count": store.count_cf(CF_MEJEPA_MUTATION_CORPUS)?,
        "verdict_cf_count": store.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?,
        "provenance_cf_count": store.count_cf(CF_MEJEPA_CORPUS_PROVENANCE)?,
        "leakage_check": {
            "failures": leakage_failures,
            "passes": leakage_passes,
        },
        "passes": ship_gate_passes,
    }))
}

fn write_stats_json(output: &Path, stats: &Value) -> CliResult<()> {
    write_json_checked(&output.join("stats.json"), stats)
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
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    let readback = fs::read_to_string(path)?;
    if readback != text {
        return Err(format!("readback mismatch after writing {}", path.display()).into());
    }
    Ok(())
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
