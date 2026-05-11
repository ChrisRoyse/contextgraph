use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::oracle::{OracleError, OracleStore};
use crate::source_patch::{mutate_task_source_patch, SourcePatchConfig, SourcePatchTask};
use crate::split::{
    assert_no_test_patch_leakage, split_corpus, CorpusEntry, SplitConfig, SplitMode, SplitReport,
    TaskAssignment,
};
use crate::swebench::{
    run_swebench_lite_oracle, SwebenchOracleConfig, SwebenchOracleResult, SwebenchPrediction,
    SwebenchPredictionMode,
};
use crate::{
    compute_corpus_sha256, CorpusProvenance, CorpusProvenanceRow, Language, MutationCategory,
    MutationError, MutationOutcome, MEJEPA_CORPUS_VERSION_V1, MEJEPA_EMBEDDER_VERSION_V1,
};

pub trait Oracle {
    fn run(&self, config: &SwebenchOracleConfig) -> Result<SwebenchOracleResult, OracleError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SwebenchDockerOracle;

impl Oracle for SwebenchDockerOracle {
    fn run(&self, config: &SwebenchOracleConfig) -> Result<SwebenchOracleResult, OracleError> {
        run_swebench_lite_oracle(config)
    }
}

#[derive(Debug, Clone)]
pub struct CorpusReport {
    pub complete: bool,
    pub entry_count: usize,
    pub split: SplitReport,
    pub provenance: Option<CorpusProvenance>,
    pub index_path: PathBuf,
}

#[derive(Debug, Error)]
pub enum MejepaCorpusError {
    #[error("{0}")]
    Mutation(#[from] MutationError),
    #[error("{0}")]
    Oracle(#[from] OracleError),
    #[error("{0}")]
    Path(#[from] context_graph_paths::PathError),
    #[error("corpus I/O failed at {context}: {source}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    #[error("unsupported corpus language {0:?}; supported set is the canonical 11-language mutation set")]
    UnsupportedLanguage(Language),
    #[error("corpus JSON failed at {context}: {message}")]
    Json { context: String, message: String },
}

impl MejepaCorpusError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Mutation(err) => err.code(),
            Self::Oracle(err) => err.code(),
            Self::Path(err) => err.code,
            Self::Io { .. } => "MEJEPA_CORPUS_IO",
            Self::UnsupportedLanguage(_) => "MEJEPA_CHUNKER_UNSUPPORTED_LANGUAGE",
            Self::Json { .. } => "MEJEPA_CORPUS_JSON",
        }
    }
}

#[derive(Debug, Clone)]
struct GeneratorConfig {
    tasks_dir: PathBuf,
    output_root: PathBuf,
    venv_python: PathBuf,
    repo_cache_dir: PathBuf,
    source_work_root: PathBuf,
    run_id_prefix: String,
    categories: Vec<MutationCategory>,
    languages: Vec<Language>,
    seed: u64,
    instance_timeout: Duration,
    overall_timeout: Duration,
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
    corpus_version: String,
    embedder_version: String,
    complete: bool,
    generated_at_unix_ms: u128,
    seed: u64,
    languages: Vec<Language>,
    tasks_dir: String,
    store_path: String,
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

pub fn generate_swe_bench_mutation_corpus(
    swe_bench_lite_root: &Path,
    output_root: &Path,
    categories: &[MutationCategory],
    languages: &[Language],
) -> Result<CorpusReport, MejepaCorpusError> {
    let oracle = SwebenchDockerOracle;
    generate_swe_bench_mutation_corpus_with_oracle(
        swe_bench_lite_root,
        output_root,
        categories,
        languages,
        &oracle,
    )
}

pub fn generate_swe_bench_mutation_corpus_with_oracle<O: Oracle>(
    swe_bench_lite_root: &Path,
    output_root: &Path,
    categories: &[MutationCategory],
    languages: &[Language],
    oracle: &O,
) -> Result<CorpusReport, MejepaCorpusError> {
    let config = GeneratorConfig {
        tasks_dir: swe_bench_lite_root.to_path_buf(),
        output_root: output_root.to_path_buf(),
        venv_python: default_venv_python(),
        repo_cache_dir: default_repo_cache_dir(),
        source_work_root: context_graph_paths::mejepa_corpus_source_work_root()?,
        run_id_prefix: "mejepa-corpus-lib".to_string(),
        categories: if categories.is_empty() {
            MutationCategory::all().to_vec()
        } else {
            categories.to_vec()
        },
        languages: if languages.is_empty() {
            vec![Language::Python]
        } else {
            languages.to_vec()
        },
        seed: 0,
        instance_timeout: Duration::from_secs(1800),
        overall_timeout: Duration::from_secs(3600),
    };
    generate_with_config(&config, oracle)
}

fn generate_with_config<O: Oracle>(
    config: &GeneratorConfig,
    oracle: &O,
) -> Result<CorpusReport, MejepaCorpusError> {
    validate_languages(&config.languages)?;
    let output_root = context_graph_paths::require_under_mejepa_corpus_root(
        &config.output_root,
        "generate.output_root",
    )?;
    let venv_python =
        context_graph_paths::require_under_data_root(&config.venv_python, "generate.venv_python")?;
    let repo_cache_dir = context_graph_paths::require_under_data_root(
        &config.repo_cache_dir,
        "generate.repo_cache_dir",
    )?;
    let source_work_root = context_graph_paths::require_under_data_root(
        &config.source_work_root,
        "generate.source_work_root",
    )?;
    if !venv_python.is_file() {
        return Err(io_error(
            &venv_python,
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "SWE-bench venv python missing",
            ),
        ));
    }
    let config = GeneratorConfig {
        output_root,
        venv_python,
        repo_cache_dir,
        source_work_root,
        ..config.clone()
    };
    ensure_fresh_output(&config.output_root)?;
    fs::create_dir_all(&config.output_root)
        .map_err(|source| io_error(&config.output_root, source))?;

    let manifests = load_task_manifests(&config.tasks_dir)?;
    let official_patches = load_official_patches(
        &config.venv_python,
        &manifests
            .iter()
            .map(|manifest| manifest.instance_id.as_str())
            .collect::<Vec<_>>(),
    )?;
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

    let store_path_rel = PathBuf::from("oracle-store");
    let store_path = config.output_root.join(&store_path_rel);
    let store = OracleStore::open(&store_path)?;
    let mut index = CorpusIndex {
        schema_version: 1,
        corpus_version: MEJEPA_CORPUS_VERSION_V1.to_string(),
        embedder_version: MEJEPA_EMBEDDER_VERSION_V1.to_string(),
        complete: false,
        generated_at_unix_ms: unix_ms(),
        seed: config.seed,
        languages: config.languages.clone(),
        tasks_dir: config.tasks_dir.display().to_string(),
        store_path: store_path_rel.display().to_string(),
        corpus_sha256: None,
        entries: Vec::new(),
        split_assignments: split.assignments.clone(),
    };
    let index_path = config.output_root.join("corpus-index.json");
    write_json(
        &index_path,
        &serde_json::to_value(&index).map_err(json_error("index"))?,
    )?;

    let source_patch_config = SourcePatchConfig {
        repo_cache_dir: config.repo_cache_dir.clone(),
        work_root: config.source_work_root.clone(),
    };
    for manifest in &manifests {
        let official_patch =
            official_patches
                .get(&manifest.instance_id)
                .ok_or_else(|| MejepaCorpusError::Json {
                    context: manifest.instance_id.clone(),
                    message: "official patch missing after load".to_string(),
                })?;
        for category in &config.categories {
            let result = generate_one_row(
                &config,
                oracle,
                manifest,
                *category,
                official_patch,
                &source_patch_config,
                &bucket_by_task,
                &store,
            );
            match result {
                Ok(entry) => {
                    index.entries.push(entry);
                    write_json(
                        &index_path,
                        &serde_json::to_value(&index).map_err(json_error("index"))?,
                    )?;
                }
                Err(err) => {
                    index.complete = false;
                    write_json(
                        &index_path,
                        &serde_json::to_value(&index).map_err(json_error("index"))?,
                    )?;
                    return Err(err);
                }
            }
        }
    }

    let provenance = build_index_provenance(&index)?;
    store.put_provenance(&provenance)?;
    store.flush()?;
    let stored_provenance = store
        .get_provenance(&index.corpus_version, &index.embedder_version)?
        .ok_or_else(|| MejepaCorpusError::Json {
            context: "provenance".to_string(),
            message: "provenance row missing immediately after RocksDB write".to_string(),
        })?;
    if stored_provenance != provenance {
        return Err(MejepaCorpusError::Json {
            context: "provenance".to_string(),
            message: "provenance readback mismatch immediately after RocksDB write".to_string(),
        });
    }
    index.corpus_sha256 = Some(provenance.corpus_sha256.clone());
    index.complete = true;
    write_json(
        &index_path,
        &serde_json::to_value(&index).map_err(json_error("index"))?,
    )?;
    Ok(CorpusReport {
        complete: true,
        entry_count: index.entries.len(),
        split,
        provenance: Some(provenance),
        index_path,
    })
}

#[allow(clippy::too_many_arguments)]
fn generate_one_row<O: Oracle>(
    config: &GeneratorConfig,
    oracle: &O,
    manifest: &TaskManifest,
    category: MutationCategory,
    official_patch: &str,
    source_patch_config: &SourcePatchConfig,
    bucket_by_task: &BTreeMap<String, String>,
    store: &OracleStore,
) -> Result<GeneratedEntry, MejepaCorpusError> {
    let seed = stable_seed(config.seed, &manifest.instance_id, category);
    let patch_outcome = mutate_task_source_patch(
        &SourcePatchTask {
            instance_id: manifest.instance_id.clone(),
            repo: manifest.repo.clone(),
            base_commit: manifest.base_commit.clone(),
        },
        category,
        official_patch,
        seed,
        source_patch_config,
    )?;
    let patch_path_rel = write_patch(&config.output_root, &manifest.instance_id, &patch_outcome)?;
    let mutation = MutationOutcome {
        category,
        mutated_source: patch_outcome.mutated_patch.clone(),
        seed,
        mutation_site: None,
    };
    let prediction = SwebenchPrediction {
        instance_id: manifest.instance_id.clone(),
        model_name_or_path: format!("mejepa-{}", category.slug()),
        model_patch: patch_outcome.mutated_patch.clone(),
    };
    let mut oracle_config = SwebenchOracleConfig::defaults_for(
        manifest.instance_id.clone(),
        docker_run_id(&format!(
            "{}-{}-{}",
            config.run_id_prefix,
            manifest.instance_id,
            category.slug()
        ))?,
        config.output_root.join("swebench-runs"),
        config.venv_python.clone(),
        SwebenchPredictionMode::Custom(prediction),
    );
    oracle_config.instance_timeout = config.instance_timeout;
    oracle_config.overall_timeout = config.overall_timeout;
    let oracle_result = oracle.run(&oracle_config)?;
    store.put_corpus_row(
        &manifest.instance_id,
        category,
        &mutation,
        &oracle_result.verdict,
    )?;
    store.flush()?;
    let (stored_mutation, stored_verdict) = store
        .get_corpus_row(&manifest.instance_id, category)?
        .ok_or_else(|| MejepaCorpusError::Json {
            context: format!("{}|{}", manifest.instance_id, category.slug()),
            message: "corpus row missing immediately after RocksDB write".to_string(),
        })?;
    if stored_mutation != mutation || stored_verdict != oracle_result.verdict {
        return Err(MejepaCorpusError::Json {
            context: format!("{}|{}", manifest.instance_id, category.slug()),
            message: "corpus row readback mismatch immediately after RocksDB write".to_string(),
        });
    }
    Ok(GeneratedEntry {
        task_id: manifest.instance_id.clone(),
        repo: manifest.repo.clone(),
        category,
        bucket: bucket_by_task
            .get(&manifest.instance_id)
            .cloned()
            .ok_or_else(|| MejepaCorpusError::Json {
                context: manifest.instance_id.clone(),
                message: "split bucket missing for generated task".to_string(),
            })?,
        patch_path: patch_path_rel.display().to_string(),
        mutation_note: patch_outcome.note,
        patch_sha256: sha256_text(&mutation.mutated_source),
        oracle_verdict_sha256: sha256_json_value(
            &serde_json::to_value(&oracle_result.verdict).map_err(json_error("verdict"))?,
        ),
        oracle_all_passed: oracle_result.verdict.all_passed(),
        oracle_exception: oracle_result
            .verdict
            .exception
            .map(|class| class.slug().to_string()),
        oracle_per_test_count: oracle_result.verdict.per_test.len(),
    })
}

fn validate_languages(languages: &[Language]) -> Result<(), MejepaCorpusError> {
    if languages.is_empty() {
        return Err(MutationError::invalid(
            "languages",
            "at least one language is required",
            "pass one or more canonical Language values from the 11-language mutation set",
        )
        .into());
    }
    for language in languages {
        if !language.is_supported() {
            return Err(MejepaCorpusError::UnsupportedLanguage(*language));
        }
    }
    Ok(())
}

fn load_task_manifests(tasks_dir: &Path) -> Result<Vec<TaskManifest>, MejepaCorpusError> {
    let mut manifests = Vec::new();
    for entry in fs::read_dir(tasks_dir).map_err(|source| io_error(tasks_dir, source))? {
        let path = entry.map_err(|source| io_error(tasks_dir, source))?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let manifest: TaskManifest = serde_json::from_str(
            &fs::read_to_string(&path).map_err(|source| io_error(&path, source))?,
        )
        .map_err(json_error(path.display().to_string()))?;
        manifests.push(manifest);
    }
    manifests.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));
    if manifests.is_empty() {
        return Err(MejepaCorpusError::Json {
            context: tasks_dir.display().to_string(),
            message: "no task manifests found".to_string(),
        });
    }
    Ok(manifests)
}

fn load_official_patches(
    venv_python: &Path,
    instance_ids: &[&str],
) -> Result<BTreeMap<String, String>, MejepaCorpusError> {
    let ids_json = serde_json::to_string(instance_ids).map_err(json_error("instance_ids"))?;
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
        .output()
        .map_err(|source| io_error(venv_python, source))?;
    if !output.status.success() {
        return Err(MejepaCorpusError::Json {
            context: venv_python.display().to_string(),
            message: format!(
                "official patch loader failed: status={:?} stdout={} stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    let patches: BTreeMap<String, String> =
        serde_json::from_slice(&output.stdout).map_err(json_error("official_patches"))?;
    for (task, patch) in &patches {
        if patch.trim().is_empty() {
            return Err(MejepaCorpusError::Json {
                context: task.clone(),
                message: "official dataset returned empty patch".to_string(),
            });
        }
    }
    Ok(patches)
}

fn write_patch(
    output_root: &Path,
    task_id: &str,
    outcome: &crate::patch_mutation::PatchMutationOutcome,
) -> Result<PathBuf, MejepaCorpusError> {
    let relative_path = PathBuf::from("patches")
        .join(task_id)
        .join(format!("{}.diff", outcome.category.slug()));
    let path = output_root.join(&relative_path);
    write_text_0600(&path, &outcome.mutated_patch)?;
    Ok(relative_path)
}

fn build_index_provenance(index: &CorpusIndex) -> Result<CorpusProvenance, MejepaCorpusError> {
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
    let mut task_ids = BTreeSet::new();
    let mut categories = Vec::new();
    for entry in &index.entries {
        task_ids.insert(entry.task_id.clone());
        categories.push(entry.category);
    }
    categories.sort_by_key(|category| category.slug());
    categories.dedup();
    let provenance = CorpusProvenance {
        corpus_version: index.corpus_version.clone(),
        embedder_version: index.embedder_version.clone(),
        generated_at_unix_ms: index.generated_at_unix_ms,
        generator_version: env!("CARGO_PKG_VERSION").to_string(),
        seed: index.seed,
        languages: index.languages.clone(),
        task_manifest_count: task_ids.len(),
        mutation_count: index.entries.len(),
        mutation_categories: categories,
        source_patch_mode: "source-backed".to_string(),
        split_mode: "instance_atomic".to_string(),
        corpus_sha256: compute_corpus_sha256(&rows)?,
        complete: true,
    };
    provenance.validate()?;
    Ok(provenance)
}

fn ensure_fresh_output(path: &Path) -> Result<(), MejepaCorpusError> {
    if !path.exists() {
        return Ok(());
    }
    if !path.is_dir() {
        return Err(io_error(
            path,
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "output path is not a directory",
            ),
        ));
    }
    let mut entries = fs::read_dir(path).map_err(|source| io_error(path, source))?;
    if entries
        .next()
        .transpose()
        .map_err(|source| io_error(path, source))?
        .is_some()
    {
        return Err(io_error(
            path,
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "output directory is not empty",
            ),
        ));
    }
    Ok(())
}

fn write_json(path: &Path, value: &Value) -> Result<(), MejepaCorpusError> {
    write_text_0600(
        path,
        &serde_json::to_string_pretty(value).map_err(json_error(path.display().to_string()))?,
    )
}

fn write_text_0600(path: &Path, text: &str) -> Result<(), MejepaCorpusError> {
    validate_relative_safe(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    }
    let mut file = File::create(path).map_err(|source| io_error(path, source))?;
    #[cfg(unix)]
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|source| io_error(path, source))?;
    file.write_all(text.as_bytes())
        .map_err(|source| io_error(path, source))?;
    file.sync_all().map_err(|source| io_error(path, source))?;
    drop(file);
    let readback = fs::read_to_string(path).map_err(|source| io_error(path, source))?;
    if readback != text {
        return Err(io_error(
            path,
            std::io::Error::other("write readback mismatch"),
        ));
    }
    Ok(())
}

fn validate_relative_safe(path: &Path) -> Result<(), MejepaCorpusError> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(io_error(
                path,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "path contains parent directory",
                ),
            ));
        }
    }
    Ok(())
}

fn docker_run_id(raw: &str) -> Result<String, OracleError> {
    let mut chars = raw.chars();
    let Some(first) = chars.next() else {
        return Err(OracleError::invalid(
            "run_id",
            "run_id is empty",
            "use a stable non-empty Docker-safe run id",
        ));
    };
    if !first.is_ascii_alphanumeric() {
        return Err(OracleError::docker_run_id_unsafe(
            raw,
            first,
            0,
            "Docker run ids must start with an ASCII alphanumeric character",
        ));
    }
    for (position, ch) in std::iter::once(first).chain(chars).enumerate() {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '-') {
            return Err(OracleError::docker_run_id_unsafe(
                raw,
                ch,
                position,
                "Docker run ids may contain only [A-Za-z0-9_.-]",
            ));
        }
    }
    Ok(raw.to_string())
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

fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn io_error(path: &Path, source: std::io::Error) -> MejepaCorpusError {
    MejepaCorpusError::Io {
        context: format!("file:{}", path.display()),
        source,
    }
}

fn json_error(context: impl Into<String>) -> impl FnOnce(serde_json::Error) -> MejepaCorpusError {
    let context = context.into();
    move |source| MejepaCorpusError::Json {
        context: context.clone(),
        message: source.to_string(),
    }
}

fn default_venv_python() -> PathBuf {
    context_graph_paths::ensure_subdir("cache/contextgraph-swebench")
        .expect("ContextGraph D-backed SWE-bench cache root must be available")
        .join("venv/bin/python")
}

fn default_repo_cache_dir() -> PathBuf {
    context_graph_paths::mejepa_corpus_repo_cache_dir()
        .expect("ContextGraph D-backed ME-JEPA corpus repo cache must be available")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_output_outside_corpus_root_returns_error() {
        let oracle = SwebenchDockerOracle;
        let err = generate_swe_bench_mutation_corpus_with_oracle(
            Path::new("tasks/swebench-lite"),
            Path::new("/mnt/d/contextgraph/fsv/contextgraph-generate-outside-corpus-root"),
            &[MutationCategory::KnownGood],
            &[Language::Python],
            &oracle,
        )
        .unwrap_err();
        assert_eq!(err.code(), "CONTEXTGRAPH_CORPUS_PATH_OUTSIDE_CORPUS_ROOT");
    }
}
