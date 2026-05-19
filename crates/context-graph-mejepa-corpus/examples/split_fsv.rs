// Manual Full State Verification for the corpus split wrapper.
//
// Sources of truth:
//   - /zfs/archive/contextgraph/fsv/context-graph-mejepa-corpus/split-evidence.json
//   - /zfs/archive/contextgraph/fsv/context-graph-mejepa-corpus/split-assignments.json
//
// This example fails closed. It writes assignment/evidence files durably,
// re-reads them, hashes them, reconstructs the assignment state from disk,
// and records before/after state for boundary cases. The happy path uses the
// real checked-in SWE-bench Lite task manifests under tasks/swebench-lite.

use context_graph_mejepa_corpus::split::{
    assert_no_test_patch_leakage, cross_cut_weights, split_corpus, split_is_leakage_free,
    CorpusEntry, SplitConfig, SplitMode, TaskAssignment,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let evidence_root = context_graph_paths::ensure_subdir("fsv/context-graph-mejepa-corpus")?;
    fs::create_dir_all(&evidence_root)?;

    let (corpus, tasks_dir, test_patch_sha_by_task) = build_real_corpus()?;
    let corpus_json = serde_json::to_string_pretty(&corpus)?;
    let corpus_sha256 = sha256_text(&corpus_json);
    let total = corpus.len();
    // The FSV's primary verifies the RepoAtomic invariants (no cross-bucket
    // repo edges) using the existing assertions. The new default is
    // InstanceAtomic; we verify that mode separately below.
    let config = SplitConfig {
        mode: SplitMode::RepoAtomic,
        ..SplitConfig::default()
    };
    let report = split_corpus(&corpus, config)
        .map_err(|e| -> Box<dyn std::error::Error> { format!("split_corpus error: {e}").into() })?;

    require_eq("assignment count", &total, &report.assignments.len())?;
    let task_ids_in: HashSet<&str> = corpus.iter().map(|e| e.task_id.as_str()).collect();
    let task_ids_out: HashSet<&str> = report
        .assignments
        .iter()
        .map(|a| a.task_id.as_str())
        .collect();
    require_eq("assigned task_id set", &task_ids_in, &task_ids_out)?;
    require_eq(
        "bucket size total",
        &total,
        &report.bucket_sizes.iter().sum::<usize>(),
    )?;
    require(
        report.bucket_sizes.iter().all(|&s| s > 0),
        "every bucket must have at least one task",
    )?;
    require(
        report.leakage_check_passed,
        "leakage_check_passed must be true",
    )?;
    require(
        split_is_leakage_free(&report),
        "split_is_leakage_free must be true",
    )?;
    let cross_cut_weights = cross_cut_weights(&report);
    require_eq("cross_cut_weights", &[0.0, 0.0, 0.0], &cross_cut_weights)?;

    let fractions = report.bucket_fractions;
    require(
        fractions[0] >= 0.55 && fractions[0] <= 0.95,
        &format!("train fraction {} must be in [0.55, 0.95]", fractions[0]),
    )?;
    require(
        fractions[1] >= 0.02 && fractions[1] <= 0.30,
        &format!(
            "calibration fraction {} must be in [0.02, 0.30]",
            fractions[1]
        ),
    )?;
    require(
        fractions[2] >= 0.02 && fractions[2] <= 0.30,
        &format!("holdout fraction {} must be in [0.02, 0.30]", fractions[2]),
    )?;

    let repo_assignments_path = evidence_root.join("split-repo-atomic-assignments.json");
    let repo_assignments_text = serde_json::to_string_pretty(&report.assignments)?;
    let repo_assignments_sha256 =
        write_text_checked(&repo_assignments_path, &repo_assignments_text)?;
    let repo_assignments_readback_text = fs::read_to_string(&repo_assignments_path)?;
    let repo_assignments_readback: Vec<TaskAssignment> =
        serde_json::from_str(&repo_assignments_readback_text)?;
    require_eq(
        "repo-atomic assignments JSON readback",
        &report.assignments,
        &repo_assignments_readback,
    )?;

    let empty_case = split_boundary_case(
        "empty_corpus",
        &[],
        SplitConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let single_repo: Vec<CorpusEntry> = (0..50)
        .map(|i| CorpusEntry {
            task_id: format!("solo__task-{i}"),
            repo: "solo".to_string(),
        })
        .collect();
    let single_repo_case = split_boundary_case(
        "single_repo_degenerate_repo_atomic",
        &single_repo,
        SplitConfig {
            mode: SplitMode::RepoAtomic,
            ..SplitConfig::default()
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let bad_fractions_case = split_boundary_case(
        "fractions_sum_1_1",
        &corpus,
        SplitConfig {
            train_fraction: 0.7,
            calibration_fraction: 0.2,
            holdout_fraction: 0.2,
            seed: 0,
            ..SplitConfig::default()
        },
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let duplicate_task_case = split_boundary_case(
        "duplicate_task_id",
        &[
            CorpusEntry {
                task_id: "dup-task".to_string(),
                repo: "repo-a".to_string(),
            },
            CorpusEntry {
                task_id: "dup-task".to_string(),
                repo: "repo-b".to_string(),
            },
            CorpusEntry {
                task_id: "unique-task".to_string(),
                repo: "repo-c".to_string(),
            },
        ],
        SplitConfig::default(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;

    let repo_to_bucket = repo_bucket_map(&report.assignments);

    // ---- Phase 0 dataset-shape report: InstanceAtomic mode (post-2026-05-09 default) ----
    //
    // RepoAtomic on Lite cannot reach 80/10/10 because django=38% / sympy=25.7%.
    // InstanceAtomic achieves the requested 80/10/10 by placing each task
    // independently. Every instance has its own test_patch SHA, so this is
    // leakage-free at the test-evaluation level.
    let instance_config = SplitConfig {
        mode: SplitMode::InstanceAtomic,
        ..SplitConfig::default()
    };
    let instance_report =
        split_corpus(&corpus, instance_config).map_err(|e| -> Box<dyn std::error::Error> {
            format!("instance-atomic split error: {e}").into()
        })?;
    let instance_target_train = (instance_config.train_fraction * total as f64).round() as usize;
    let instance_target_cal =
        (instance_config.calibration_fraction * total as f64).round() as usize;
    let instance_target_hold = total.saturating_sub(instance_target_train + instance_target_cal);
    require_eq(
        "instance-atomic bucket sizes",
        &[
            instance_target_train,
            instance_target_cal,
            instance_target_hold,
        ],
        &instance_report.bucket_sizes,
    )?;
    require(
        instance_report.leakage_check_passed,
        "instance-atomic leakage_check_passed must be true",
    )?;
    require(
        instance_report.repo_assignments.is_empty(),
        "instance-atomic produces no per-repo assignments",
    )?;
    assert_no_test_patch_leakage(&instance_report.assignments, |task_id| {
        test_patch_sha_by_task.get(task_id).map(String::as_str)
    })
    .map_err(|e| -> Box<dyn std::error::Error> {
        format!("instance-atomic official_test_patch_sha256 leakage check failed: {e}").into()
    })?;

    let assignments_path = evidence_root.join("split-assignments.json");
    let assignments_text = serde_json::to_string_pretty(&instance_report.assignments)?;
    let assignments_sha256 = write_text_checked(&assignments_path, &assignments_text)?;
    let assignments_readback_text = fs::read_to_string(&assignments_path)?;
    let assignments_readback: Vec<TaskAssignment> =
        serde_json::from_str(&assignments_readback_text)?;
    require_eq(
        "instance-atomic assignments JSON readback",
        &instance_report.assignments,
        &assignments_readback,
    )?;

    let missing_test_patch_sha_case = test_patch_leakage_boundary_case(
        "missing_test_patch_sha",
        &instance_report.assignments,
        &BTreeMap::new(),
        "MEJEPA_CORPUS_INVALID_INPUT",
    )?;
    let duplicate_sha_by_task: BTreeMap<String, String> = instance_report
        .assignments
        .iter()
        .map(|assignment| {
            (
                assignment.task_id.clone(),
                "sha256:duplicate-test-patch".to_string(),
            )
        })
        .collect();
    let duplicate_test_patch_sha_case = test_patch_leakage_boundary_case(
        "duplicate_test_patch_sha_cross_bucket",
        &instance_report.assignments,
        &duplicate_sha_by_task,
        "MEJEPA_CORPUS_LEAKAGE_DETECTED",
    )?;

    let unique_test_patch_sha_count = test_patch_sha_by_task
        .values()
        .collect::<HashSet<_>>()
        .len();

    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_root.join("split-evidence.json").display().to_string(),
            "assignments_json": assignments_path.display().to_string(),
            "assignments_sha256": assignments_sha256,
            "repo_atomic_assignments_json": repo_assignments_path.display().to_string(),
            "repo_atomic_assignments_sha256": repo_assignments_sha256,
            "task_manifest_dir": tasks_dir.display().to_string(),
            "official_test_patch_sha256_source": "tasks/swebench-lite/*.json:official_test_patch_sha256",
        },
        "input_corpus_sha256": corpus_sha256,
        "corpus_size": total,
        "repo_atomic": {
            "mode": "repo_atomic",
            "repo_count": report.repo_assignments.len(),
            "bucket_sizes": report.bucket_sizes,
            "bucket_fractions": report.bucket_fractions,
            "cross_cut_weights": cross_cut_weights,
            "leakage_check_passed": report.leakage_check_passed,
            "repo_to_bucket": repo_to_bucket,
            "note_on_dataset_shape": "300 tasks across 12 repos with django=38% and sympy=25.7%; 240/30/30 unattainable via whole-repo atomicity",
        },
        "instance_atomic": {
            "mode": "instance_atomic",
            "bucket_sizes": instance_report.bucket_sizes,
            "bucket_fractions": instance_report.bucket_fractions,
            "leakage_check_passed": instance_report.leakage_check_passed,
            "achieves_80_10_10": instance_report.bucket_sizes == [240, 30, 30],
            "test_patch_sha_leakage_check_passed": true,
            "official_test_patch_sha256_count": test_patch_sha_by_task.len(),
            "unique_official_test_patch_sha256_count": unique_test_patch_sha_count,
            "note_on_leakage": "each SWE-bench Lite instance has a unique official_test_patch_sha256 and assert_no_test_patch_leakage verified no identical evaluator test patch crosses train/calibration/holdout buckets",
        },
        "edge_cases": {
            "empty_corpus": empty_case,
            "single_repo_degenerate_repo_atomic": single_repo_case,
            "fractions_sum_1_1": bad_fractions_case,
            "duplicate_task_id": duplicate_task_case,
            "missing_test_patch_sha": missing_test_patch_sha_case,
            "duplicate_test_patch_sha_cross_bucket": duplicate_test_patch_sha_case,
        },
        "passes": true,
    });

    let evidence_path = evidence_root.join("split-evidence.json");
    let evidence_text = serde_json::to_string_pretty(&evidence)?;
    let evidence_sha256 = write_text_checked(&evidence_path, &evidence_text)?;
    // Disk-vs-memory equality is already enforced byte-for-byte by
    // `write_text_checked` (it reads back and compares the bytes). We
    // additionally re-parse the file as JSON to confirm it is structurally
    // valid, but we deliberately do NOT compare the parsed `Value` with the
    // in-memory `Value` because f64 → JSON-text → f64 round-trip can shift
    // the bit-pattern by 1 ULP for some values (e.g. `0.10333333333333333`).
    // The on-disk text is the authoritative record; the byte equality
    // already proved we wrote what we intended.
    let evidence_readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        evidence_readback.is_object(),
        "evidence file must parse to a JSON object",
    )?;

    println!("Split FSV evidence: {}", evidence_path.display());
    println!("Split assignments JSON: {}", assignments_path.display());
    println!("Evidence SHA-256: {evidence_sha256}");
    println!("{}", evidence_text);
    Ok(())
}

fn build_real_corpus() -> FsvResult<(Vec<CorpusEntry>, PathBuf, BTreeMap<String, String>)> {
    let tasks_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tasks/swebench-lite")
        .canonicalize()?;
    let mut entries = Vec::new();
    let mut test_patch_sha_by_task = BTreeMap::new();
    for dir_entry in fs::read_dir(&tasks_dir)? {
        let dir_entry = dir_entry?;
        let path = dir_entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path)?;
        let manifest: Value = serde_json::from_str(&text)?;
        let instance_id = manifest
            .get("instance_id")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("task manifest missing instance_id: {}", path.display()))?;
        let repo = manifest
            .get("repo")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("task manifest missing repo: {}", path.display()))?;
        let test_patch_sha = manifest
            .get("official_test_patch_sha256")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                format!(
                    "task manifest missing official_test_patch_sha256: {}",
                    path.display()
                )
            })?;
        require(
            test_patch_sha.starts_with("sha256:"),
            &format!(
                "official_test_patch_sha256 must be prefixed with sha256: in {}",
                path.display()
            ),
        )?;
        entries.push(CorpusEntry {
            task_id: instance_id.to_string(),
            repo: repo.to_string(),
        });
        test_patch_sha_by_task.insert(instance_id.to_string(), test_patch_sha.to_string());
    }
    entries.sort_by(|a, b| a.task_id.cmp(&b.task_id));
    require(
        !entries.is_empty(),
        &format!("no task manifests found under {}", tasks_dir.display()),
    )?;
    require_eq(
        "test_patch_sha_by_task count",
        &entries.len(),
        &test_patch_sha_by_task.len(),
    )?;
    Ok((entries, tasks_dir, test_patch_sha_by_task))
}

fn split_boundary_case(
    name: &str,
    corpus: &[CorpusEntry],
    config: SplitConfig,
    expected_code: &str,
) -> FsvResult<Value> {
    let corpus_json = serde_json::to_string(corpus)?;
    let before_state = json!({
        "name": name,
        "corpus_size": corpus.len(),
        "repo_count": corpus.iter().map(|entry| entry.repo.as_str()).collect::<HashSet<_>>().len(),
        "corpus_sha256": sha256_text(&corpus_json),
        "config": config,
    });
    let after_state = match split_corpus(corpus, config) {
        Err(error) => {
            require_eq(name, &expected_code, &error.code())?;
            json!({
                "result": "error",
                "code": error.code(),
                "message": error.to_string(),
            })
        }
        Ok(report) => {
            return Err(format!(
                "boundary case {name} unexpectedly succeeded with bucket sizes {:?}",
                report.bucket_sizes
            )
            .into());
        }
    };
    Ok(json!({
        "before_state": before_state,
        "after_state": after_state,
        "expected_code": expected_code,
        "actual_code": after_state["code"],
        "passes": true,
    }))
}

fn test_patch_leakage_boundary_case(
    name: &str,
    assignments: &[TaskAssignment],
    sha_by_task: &BTreeMap<String, String>,
    expected_code: &str,
) -> FsvResult<Value> {
    let before_state = json!({
        "name": name,
        "assignment_count": assignments.len(),
        "sha_map_count": sha_by_task.len(),
        "unique_sha_count": sha_by_task.values().collect::<HashSet<_>>().len(),
    });
    let after_state = match assert_no_test_patch_leakage(assignments, |task_id| {
        sha_by_task.get(task_id).map(String::as_str)
    }) {
        Err(error) => {
            require_eq(name, &expected_code, &error.code())?;
            json!({
                "result": "error",
                "code": error.code(),
                "message": error.to_string(),
            })
        }
        Ok(()) => {
            return Err(format!(
                "boundary case {name} unexpectedly passed test-patch leakage check"
            )
            .into());
        }
    };
    Ok(json!({
        "before_state": before_state,
        "after_state": after_state,
        "expected_code": expected_code,
        "actual_code": after_state["code"],
        "passes": true,
    }))
}

fn repo_bucket_map(assignments: &[TaskAssignment]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for assignment in assignments {
        map.entry(assignment.repo.clone())
            .or_insert_with(|| assignment.bucket.slug().to_string());
    }
    map
}

fn write_text_checked(path: &Path, text: &str) -> FsvResult<String> {
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
    require_eq(
        &format!("readback {}", path.display()),
        &text,
        &readback.as_str(),
    )?;
    Ok(sha256_text(&readback))
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn require(condition: bool, label: &str) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("split FSV invariant failed: {label}").into())
    }
}

fn require_eq<T>(label: &str, expected: &T, actual: &T) -> FsvResult<()>
where
    T: PartialEq + Debug + ?Sized,
{
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "split FSV invariant failed for {label}: expected {expected:?}, actual {actual:?}"
        )
        .into())
    }
}
