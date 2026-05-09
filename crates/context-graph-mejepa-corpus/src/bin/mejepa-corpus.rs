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
    run_swebench_lite_oracle, SwebenchOracleConfig, SwebenchPrediction, SwebenchPredictionMode,
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
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

type CliResult<T> = Result<T, Box<dyn std::error::Error>>;

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
}

#[derive(Parser, Debug)]
struct GenerateArgs {
    #[arg(long)]
    output: PathBuf,
    #[arg(long, default_value = "tasks/swebench-lite")]
    tasks_dir: PathBuf,
    #[arg(
        long,
        default_value = "/home/cabdru/.cache/contextgraph-swebench/venv/bin/python"
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
    #[arg(
        long,
        default_value = "/home/cabdru/.cache/contextgraph-mejepa-corpus/repos"
    )]
    repo_cache_dir: PathBuf,
    #[arg(long, default_value = "/tmp/contextgraph-mejepa-corpus-source-work")]
    source_work_root: PathBuf,
    #[arg(long, default_value = "mejepa-corpus")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
}

#[derive(Parser, Debug)]
struct VerifyArgs {
    #[arg(long)]
    corpus: PathBuf,
    #[arg(long, default_value_t = 0.05)]
    sample_fraction: f64,
    #[arg(
        long,
        default_value = "/home/cabdru/.cache/contextgraph-swebench/venv/bin/python"
    )]
    venv_python: PathBuf,
    #[arg(long, default_value = "mejepa-corpus-verify")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
}

#[derive(Parser, Debug)]
struct StatsArgs {
    #[arg(long)]
    corpus: PathBuf,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlannedMutation {
    manifest: TaskManifest,
    outcome: PatchMutationOutcome,
}

fn main() -> CliResult<()> {
    match Cli::parse().command {
        CommandKind::Generate(args) => generate(args),
        CommandKind::Verify(args) => verify(args),
        CommandKind::Stats(args) => stats(args),
    }
}

fn generate(args: GenerateArgs) -> CliResult<()> {
    validate_generate_args(&args)?;
    let categories = parse_categories(&args.categories)?;
    let languages = parse_languages(&args.languages)?;
    let interrupted = install_shutdown_flag()?;
    let manifests = load_task_manifests(&args.tasks_dir)?;
    let selected = select_manifests(&manifests, &args.instance_ids, args.max_tasks)?;
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

    let mut planned_mutations = Vec::new();
    for manifest in &selected {
        let official_patch = patch_by_task.get(&manifest.instance_id).ok_or_else(|| {
            format!(
                "official dataset patch missing after loader for {}",
                manifest.instance_id
            )
        })?;
        for category in &categories {
            let seed = stable_seed(args.seed, &manifest.instance_id, *category);
            let outcome = mutate_task_source_patch(
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
            planned_mutations.push(PlannedMutation {
                manifest: manifest.clone(),
                outcome,
            });
        }
    }

    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;
    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = args.output.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;

    let mut index = CorpusIndex {
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
        entries: Vec::new(),
        split_assignments: split.assignments.clone(),
    };
    write_index(&args.output, &index)?;

    for planned in &planned_mutations {
        if interrupted.load(Ordering::SeqCst) {
            write_index(&args.output, &index)?;
            return Err(
                "generation interrupted by SIGINT before starting next corpus row; index remains complete=false"
                    .into(),
            );
        }
        let manifest = &planned.manifest;
        let patch_outcome = &planned.outcome;
        let category = patch_outcome.category;
        let seed = patch_outcome.seed;
        let (_patch_path, patch_path_rel) =
            write_mutated_patch(&args.output, &manifest.instance_id, patch_outcome)?;
        let patch_sha256 = sha256_text(&patch_outcome.mutated_patch);
        let mutation = MutationOutcome {
            category,
            mutated_source: patch_outcome.mutated_patch.clone(),
            seed,
            mutation_site: None,
        };
        let verdict = run_oracle_for_patch(&args, manifest, category, patch_outcome)?;
        store.put_corpus_row(&manifest.instance_id, category, &mutation, &verdict)?;
        store.flush()?;
        let oracle_verdict_sha256 = sha256_json_value(&serde_json::to_value(&verdict)?);

        let entry = GeneratedEntry {
            task_id: manifest.instance_id.clone(),
            repo: manifest.repo.clone(),
            category,
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
        write_index(&args.output, &index)?;
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

fn verify(args: VerifyArgs) -> CliResult<()> {
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
        let sampled_entries = deterministic_sample(
            &index.entries,
            args.sample_fraction,
            provenance.corpus_sha256.as_str(),
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

fn stats(args: StatsArgs) -> CliResult<()> {
    let index = read_index(&args.corpus)?;
    validate_complete_index(&index)?;
    let store_path = resolve_corpus_path(&args.corpus, &index.store_path)?;
    let store = OracleStore::open(&store_path)?;
    let stats = build_stats_json(&args.corpus, &index, &store)?;
    println!("{}", serde_json::to_string_pretty(&stats)?);
    Ok(())
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
    validate_docker_run_id_component(&args.run_id_prefix)?;
    Ok(())
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
    max_tasks: Option<usize>,
) -> CliResult<Vec<TaskManifest>> {
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
    MutationCategory::from_slug(value).ok_or_else(|| format!("unknown mutation category `{value}`").into())
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

fn run_oracle_for_patch(
    args: &GenerateArgs,
    manifest: &TaskManifest,
    category: MutationCategory,
    outcome: &PatchMutationOutcome,
) -> CliResult<OracleVerdict> {
    let prediction = SwebenchPrediction {
        instance_id: manifest.instance_id.clone(),
        model_name_or_path: format!("mejepa-{}", category.slug()),
        model_patch: outcome.mutated_patch.clone(),
    };
    let run_id = docker_run_id(&format!(
        "{}-{}-{}",
        args.run_id_prefix,
        manifest.instance_id,
        category.slug()
    ))?;
    let mut config = SwebenchOracleConfig::defaults_for(
        manifest.instance_id.clone(),
        run_id,
        args.output.join("swebench-runs"),
        args.venv_python.clone(),
        SwebenchPredictionMode::Custom(prediction),
    );
    config.instance_timeout = Duration::from_secs(args.instance_timeout_secs);
    config.overall_timeout = Duration::from_secs(args.overall_timeout_secs);
    let result = run_swebench_lite_oracle(&config)?;
    Ok(result.verdict)
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
    let provenance = store.get_provenance(&index.corpus_version, &index.embedder_version)?;
    let leakage_failures = split_leakage_failures(index);
    let leakage_passes = leakage_failures.is_empty();
    Ok(json!({
        "source_of_truth": {
            "index": output.join("index.json").display().to_string(),
            "store": resolve_corpus_path(output, &index.store_path)?.display().to_string(),
        },
        "provenance": provenance,
        "total_entries": index.entries.len(),
        "task_manifest_count": index.split_assignments.len(),
        "expected_full_phase0_entries": expected_full_entries,
        "full_phase0_complete": index.entries.len() == expected_full_entries,
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
        "passes": leakage_passes,
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

fn stable_seed(global_seed: u64, task_id: &str, category: MutationCategory) -> u64 {
    let digest =
        Sha256::digest(format!("{}|{}|{}", global_seed, task_id, category.slug()).as_bytes());
    u64::from_le_bytes(digest[0..8].try_into().expect("slice has 8 bytes"))
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
