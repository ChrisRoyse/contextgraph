use clap::{Parser, Subcommand};
use context_graph_mejepa_corpus::oracle::{
    OracleStore, CF_MEJEPA_MUTATION_CORPUS, CF_MEJEPA_ORACLE_VERDICTS,
};
use context_graph_mejepa_corpus::patch_mutation::{
    mutate_unified_diff, PatchMutationConfig, PatchMutationOutcome,
};
use context_graph_mejepa_corpus::split::{
    assert_no_test_patch_leakage, split_corpus, CorpusEntry, SplitConfig, SplitMode, TaskAssignment,
};
use context_graph_mejepa_corpus::swebench::{
    run_swebench_lite_oracle, SwebenchOracleConfig, SwebenchPrediction, SwebenchPredictionMode,
};
use context_graph_mejepa_corpus::{MutationCategory, MutationOutcome};
use context_graph_mejepa_instruments::OracleVerdict;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
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
    #[arg(long)]
    max_tasks: Option<usize>,
    #[arg(long, default_value = "mejepa-corpus")]
    run_id_prefix: String,
    #[arg(long, default_value_t = 1800)]
    instance_timeout_secs: u64,
    #[arg(long, default_value_t = 3600)]
    overall_timeout_secs: u64,
    #[arg(long)]
    alternate_wrong_file_path: Option<String>,
}

#[derive(Parser, Debug)]
struct VerifyArgs {
    #[arg(long)]
    corpus: PathBuf,
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
    official_test_patch_sha256: String,
    oracle_patch_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusIndex {
    schema_version: u32,
    generated_at_unix_ms: u128,
    tasks_dir: String,
    store_path: String,
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
    }
}

fn generate(args: GenerateArgs) -> CliResult<()> {
    validate_generate_args(&args)?;
    ensure_output_directory_ready(&args.output)?;
    fs::create_dir_all(&args.output)?;
    let store_path = args.output.join("oracle-store");
    let store = OracleStore::open(&store_path)?;
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
    let categories = parse_categories(&args.categories)?;
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

    let mut index = CorpusIndex {
        schema_version: 1,
        generated_at_unix_ms: unix_ms(),
        tasks_dir: args.tasks_dir.display().to_string(),
        store_path: store_path.display().to_string(),
        entries: Vec::new(),
        split_assignments: split.assignments.clone(),
    };
    write_index(&args.output, &index)?;

    for manifest in &selected {
        let official_patch = patch_by_task.get(&manifest.instance_id).ok_or_else(|| {
            format!(
                "official dataset patch missing after loader for {}",
                manifest.instance_id
            )
        })?;
        for category in &categories {
            let seed = stable_seed(&manifest.instance_id, *category);
            let patch_outcome = mutate_unified_diff(
                *category,
                official_patch,
                PatchMutationConfig {
                    seed,
                    alternate_path: args.alternate_wrong_file_path.clone(),
                },
            )
            .map_err(|err| {
                format!(
                    "failed to mutate official patch for task={} category={}: {} ({})",
                    manifest.instance_id,
                    category.slug(),
                    err,
                    err.code()
                )
            })?;
            let patch_path =
                write_mutated_patch(&args.output, &manifest.instance_id, &patch_outcome)?;
            let patch_sha256 = sha256_text(&patch_outcome.mutated_patch);
            let mutation = MutationOutcome {
                category: *category,
                mutated_source: patch_outcome.mutated_patch.clone(),
                seed,
                mutation_site: None,
            };
            store.put_mutation(&manifest.instance_id, *category, &mutation)?;
            store.flush()?;

            let verdict = run_oracle_for_patch(&args, manifest, *category, &patch_outcome)?;
            store.put_verdict(&manifest.instance_id, *category, &verdict)?;
            store.flush()?;
            let oracle_verdict_sha256 = sha256_json_value(&serde_json::to_value(&verdict)?);

            let entry = GeneratedEntry {
                task_id: manifest.instance_id.clone(),
                repo: manifest.repo.clone(),
                category: *category,
                bucket: bucket_by_task
                    .get(&manifest.instance_id)
                    .cloned()
                    .ok_or("selected task missing split bucket")?,
                patch_path: patch_path.display().to_string(),
                patch_sha256,
                oracle_verdict_sha256,
                oracle_all_passed: verdict.all_passed(),
                oracle_exception: verdict.exception.map(|class| class.slug().to_string()),
                oracle_per_test_count: verdict.per_test.len(),
            };
            index.entries.push(entry);
            write_index(&args.output, &index)?;
        }
    }
    write_stats_json(&args.output, &index, &store)?;
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
    let index = read_index(&args.corpus)?;
    let store = OracleStore::open(Path::new(&index.store_path))?;
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in &index.entries {
        let key = format!("{}|{}", entry.task_id, entry.category.slug());
        if !seen.insert(key.clone()) {
            failures.push(json!({"key": key, "error": "duplicate index entry"}));
        }
        let patch_text = fs::read_to_string(&entry.patch_path)?;
        let actual_hash = sha256_text(&patch_text);
        if actual_hash != entry.patch_sha256 {
            failures.push(json!({
                "key": key,
                "error": "patch hash mismatch",
                "expected": entry.patch_sha256,
                "actual": actual_hash,
            }));
        }
        match store.get_mutation(&entry.task_id, entry.category)? {
            Some(mutation) if sha256_text(&mutation.mutated_source) == entry.patch_sha256 => {}
            Some(_) => {
                failures.push(json!({"key": key, "error": "RocksDB mutation hash mismatch"}))
            }
            None => failures.push(json!({"key": key, "error": "missing RocksDB mutation row"})),
        }
        match store.get_verdict(&entry.task_id, entry.category)? {
            Some(verdict) => {
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
            None => failures.push(json!({"key": key, "error": "missing RocksDB verdict row"})),
        }
    }
    let mutation_cf_count = store.count_cf(CF_MEJEPA_MUTATION_CORPUS)?;
    let verdict_cf_count = store.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?;
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
    let passes = failures.is_empty();
    let evidence = json!({
        "source_of_truth": {
            "index": args.corpus.join("index.json").display().to_string(),
            "store": index.store_path,
        },
        "entry_count": index.entries.len(),
        "mutation_cf_count": mutation_cf_count,
        "verdict_cf_count": verdict_cf_count,
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
    let store = OracleStore::open(Path::new(&index.store_path))?;
    write_stats_json(&args.corpus, &index, &store)?;
    println!("{}", fs::read_to_string(args.corpus.join("stats.json"))?);
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
    MutationCategory::all()
        .into_iter()
        .find(|category| category.slug() == value)
        .ok_or_else(|| format!("unknown mutation category `{value}`").into())
}

fn write_mutated_patch(
    output: &Path,
    task_id: &str,
    outcome: &PatchMutationOutcome,
) -> CliResult<PathBuf> {
    let dir = output.join("mutations").join(task_id);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.patch", outcome.category.slug()));
    write_text_checked(&path, &outcome.mutated_patch)?;
    Ok(path)
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

fn write_index(output: &Path, index: &CorpusIndex) -> CliResult<()> {
    write_json_checked(&output.join("index.json"), &serde_json::to_value(index)?)
}

fn read_index(corpus: &Path) -> CliResult<CorpusIndex> {
    Ok(serde_json::from_str(&fs::read_to_string(
        corpus.join("index.json"),
    )?)?)
}

fn write_stats_json(output: &Path, index: &CorpusIndex, store: &OracleStore) -> CliResult<()> {
    let mut by_category = BTreeMap::<String, usize>::new();
    let mut by_oracle = BTreeMap::<String, usize>::new();
    for entry in &index.entries {
        *by_category
            .entry(entry.category.slug().to_string())
            .or_default() += 1;
        let key = if entry.oracle_all_passed {
            "pass"
        } else {
            "fail_or_error"
        };
        *by_oracle.entry(key.to_string()).or_default() += 1;
    }
    let stats = json!({
        "source_of_truth": {
            "index": output.join("index.json").display().to_string(),
            "store": index.store_path,
        },
        "total_entries": index.entries.len(),
        "by_category": by_category,
        "by_oracle": by_oracle,
        "mutation_cf_count": store.count_cf(CF_MEJEPA_MUTATION_CORPUS)?,
        "verdict_cf_count": store.count_cf(CF_MEJEPA_ORACLE_VERDICTS)?,
        "passes": true,
    });
    write_json_checked(&output.join("stats.json"), &stats)
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

fn stable_seed(task_id: &str, category: MutationCategory) -> u64 {
    let digest = Sha256::digest(format!("{}|{}", task_id, category.slug()).as_bytes());
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
