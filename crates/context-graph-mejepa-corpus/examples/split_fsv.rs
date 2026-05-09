// Manual Full State Verification for the corpus split wrapper.
//
// Sources of truth:
//   - /tmp/contextgraph-mejepa-corpus-fsv/split-evidence.json
//   - /tmp/contextgraph-mejepa-corpus-fsv/split-assignments.json
//
// This example fails closed. It writes assignment/evidence files durably,
// re-reads them, hashes them, reconstructs the assignment state from disk,
// and records before/after state for boundary cases.

use context_graph_mejepa_corpus::split::{
    cross_cut_weights, split_corpus, split_is_leakage_free, CorpusEntry, SplitConfig,
    TaskAssignment,
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
    let evidence_root = PathBuf::from("/tmp/contextgraph-mejepa-corpus-fsv");
    fs::create_dir_all(&evidence_root)?;

    let corpus = build_synthetic_corpus();
    let corpus_json = serde_json::to_string_pretty(&corpus)?;
    let corpus_sha256 = sha256_text(&corpus_json);
    let total = corpus.len();
    let config = SplitConfig::default();
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

    let assignments_path = evidence_root.join("split-assignments.json");
    let assignments_text = serde_json::to_string_pretty(&report.assignments)?;
    let assignments_sha256 = write_text_checked(&assignments_path, &assignments_text)?;
    let assignments_readback_text = fs::read_to_string(&assignments_path)?;
    let assignments_readback: Vec<TaskAssignment> =
        serde_json::from_str(&assignments_readback_text)?;
    require_eq(
        "assignments JSON readback",
        &report.assignments,
        &assignments_readback,
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
        "single_repo_degenerate",
        &single_repo,
        SplitConfig::default(),
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
    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_root.join("split-evidence.json").display().to_string(),
            "assignments_json": assignments_path.display().to_string(),
            "assignments_sha256": assignments_sha256,
        },
        "input_corpus_sha256": corpus_sha256,
        "corpus_size": total,
        "repo_count": report.repo_assignments.len(),
        "bucket_sizes": report.bucket_sizes,
        "bucket_fractions": report.bucket_fractions,
        "cross_cut_weights": cross_cut_weights,
        "leakage_check_passed": report.leakage_check_passed,
        "repo_to_bucket": repo_to_bucket,
        "edge_cases": {
            "empty_corpus": empty_case,
            "single_repo_degenerate": single_repo_case,
            "fractions_sum_1_1": bad_fractions_case,
            "duplicate_task_id": duplicate_task_case,
        },
        "passes": true,
    });

    let evidence_path = evidence_root.join("split-evidence.json");
    let evidence_text = serde_json::to_string_pretty(&evidence)?;
    let evidence_sha256 = write_text_checked(&evidence_path, &evidence_text)?;
    let evidence_readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require_eq("evidence JSON readback", &evidence, &evidence_readback)?;

    println!("Split FSV evidence: {}", evidence_path.display());
    println!("Split assignments JSON: {}", assignments_path.display());
    println!("Evidence SHA-256: {evidence_sha256}");
    println!("{}", evidence_text);
    Ok(())
}

fn build_synthetic_corpus() -> Vec<CorpusEntry> {
    let plan: &[(&str, usize)] = &[
        ("astropy", 22),
        ("django", 40),
        ("flask", 4),
        ("matplotlib", 18),
        ("mwaskom__seaborn", 6),
        ("pallets__click", 3),
        ("psf__requests", 6),
        ("pvlib__pvlib-python", 4),
        ("pydata__xarray", 12),
        ("pydicom__pydicom", 5),
        ("pylint-dev__astroid", 5),
        ("pylint-dev__pylint", 13),
        ("pyvista__pyvista", 4),
        ("scikit-learn__scikit-learn", 28),
        ("sphinx-doc__sphinx", 19),
        ("sqlfluff__sqlfluff", 12),
        ("sympy__sympy", 75),
        ("hpcugent__easybuild-easyconfigs", 4),
        ("conan-io__conan", 5),
        ("scrapy", 5),
        ("requests-toolbelt", 3),
        ("pytest-dev__pytest", 12),
        ("celery", 4),
        ("paramiko", 3),
        ("redis-py", 8),
    ];
    let total: usize = plan.iter().map(|(_, c)| c).sum();
    assert_eq!(total, 300);
    let mut entries = Vec::with_capacity(total);
    for (repo, count) in plan {
        for i in 0..*count {
            entries.push(CorpusEntry {
                task_id: format!("{repo}__task-{i:04}"),
                repo: (*repo).to_string(),
            });
        }
    }
    entries
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
