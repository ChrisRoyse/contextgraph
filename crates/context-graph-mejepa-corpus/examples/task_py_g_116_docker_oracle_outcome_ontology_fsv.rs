use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use context_graph_mejepa_cf::CF_MEJEPA_DOCKER_OUTCOME_LABELS;
use context_graph_mejepa_corpus::docker_outcome_labels::{
    materialize_docker_outcome_labels, read_all_rows, DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;

type FsvResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

fn main() -> FsvResult<()> {
    let args = Args::parse()?;
    fs::create_dir_all(&args.fsv_root)?;
    let db_path = args.fsv_root.join("docker-outcome-labels.rocksdb");
    if db_path.exists() {
        fs::remove_dir_all(&db_path)?;
    }
    let db = open_db(&db_path, true)?;
    let report = materialize_docker_outcome_labels(&args.corpus_root, &db, &db_path)?;
    db.flush_cf(cf(&db)?)?;
    drop(db);

    let reopened = open_db(&db_path, false)?;
    let rows = read_all_rows(&reopened)?;
    let readback_category_counts = category_counts(&rows);
    let readback_label_counts = label_counts(&rows);
    let readback_report = json!({
        "schemaVersion": DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
        "rowCount": rows.len(),
        "categoryCounts": readback_category_counts,
        "labelCounts": readback_label_counts,
        "resolvedIndexMismatchCount": rows.iter().filter(|row| row.resolved_index_mismatch).count(),
        "dockerReportMissingCount": rows.iter().filter(|row| row.docker_report_missing).count(),
        "oraclePerTestCountMismatchCount": rows.iter().filter(|row| row.test_phase_vector.total() != row.oracle_per_test_count).count(),
    });
    let all_passed = report.all_acceptance_passed()
        && rows.len() as u64 == report.rows_persisted
        && readback_category_counts == report.expected_category_counts
        && readback_report["resolvedIndexMismatchCount"] == json!(0)
        && readback_report["dockerReportMissingCount"] == json!(0)
        && readback_report["oraclePerTestCountMismatchCount"] == json!(0);
    let fsv = json!({
        "artifactKind": "task_py_g_116_docker_oracle_outcome_ontology_fsv",
        "schemaVersion": DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
        "allPassed": all_passed,
        "sourceOfTruth": {
            "corpusRoot": args.corpus_root,
            "dbPath": db_path,
            "labelCf": CF_MEJEPA_DOCKER_OUTCOME_LABELS,
        },
        "materialization": report,
        "readback": readback_report,
        "checks": {
            "uniqueKeys2400": rows.len() == 2400,
            "categoryCountsMatchIndex": readback_category_counts == report.expected_category_counts,
            "resolvedMatchesIndex": readback_report["resolvedIndexMismatchCount"] == json!(0),
            "allReportsPresent": readback_report["dockerReportMissingCount"] == json!(0),
            "testPhaseCountsMatchOraclePerTest": readback_report["oraclePerTestCountMismatchCount"] == json!(0),
            "predictionPatchRowsAttached": report.prediction_patch_rows_attached,
        },
    });
    let report_path = args.fsv_root.join("docker_outcome_ontology_fsv.json");
    let readback_path = args.fsv_root.join("docker_outcome_ontology_readback.json");
    fs::write(&report_path, serde_json::to_vec_pretty(&fsv)?)?;
    fs::write(&readback_path, serde_json::to_vec_pretty(&readback_report)?)?;
    println!("{}", serde_json::to_string_pretty(&fsv)?);
    if !all_passed {
        return Err(invalid("TASK-PY-G-116 FSV failed"));
    }
    Ok(())
}

#[derive(Debug)]
struct Args {
    corpus_root: PathBuf,
    fsv_root: PathBuf,
}

impl Args {
    fn parse() -> FsvResult<Self> {
        let mut corpus_root = None;
        let mut fsv_root = None;
        let mut iter = env::args().skip(1);
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--corpus-root" => corpus_root = iter.next().map(PathBuf::from),
                "--fsv-root" => fsv_root = iter.next().map(PathBuf::from),
                _ => return Err(invalid(format!("unknown arg {arg}"))),
            }
        }
        Ok(Self {
            corpus_root: corpus_root.ok_or_else(|| invalid("--corpus-root is required"))?,
            fsv_root: fsv_root.ok_or_else(|| invalid("--fsv-root is required"))?,
        })
    }
}

fn open_db(path: &Path, create: bool) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(create);
    opts.create_missing_column_families(create);
    let cf_opts = Options::default();
    Ok(DB::open_cf_descriptors(
        &opts,
        path,
        vec![ColumnFamilyDescriptor::new(
            CF_MEJEPA_DOCKER_OUTCOME_LABELS,
            cf_opts,
        )],
    )?)
}

fn cf(db: &DB) -> FsvResult<&rocksdb::ColumnFamily> {
    db.cf_handle(CF_MEJEPA_DOCKER_OUTCOME_LABELS)
        .ok_or_else(|| invalid(format!("missing CF {CF_MEJEPA_DOCKER_OUTCOME_LABELS}")))
}

fn category_counts(
    rows: &[context_graph_mejepa_corpus::docker_outcome_labels::DockerOutcomeLabelRow],
) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for row in rows {
        *counts.entry(row.mutation_category.clone()).or_insert(0) += 1;
    }
    counts
}

fn label_counts(
    rows: &[context_graph_mejepa_corpus::docker_outcome_labels::DockerOutcomeLabelRow],
) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    for row in rows {
        for (label, active) in [
            ("oracle_pass", row.oracle_pass),
            ("oracle_fail_or_error", row.oracle_fail_or_error),
            ("harness_exception_or_error", row.harness_exception_or_error),
            ("docker_resolved_clean", row.docker_resolved_clean),
            (
                "target_fail_to_pass_tests_failed",
                row.target_fail_to_pass_tests_failed,
            ),
            (
                "regression_pass_to_pass_tests_failed",
                row.regression_pass_to_pass_tests_failed,
            ),
            (
                "target_and_regression_tests_failed",
                row.target_and_regression_tests_failed,
            ),
            ("patch_apply_failure", row.patch_apply_failure),
            ("docker_report_missing", row.docker_report_missing),
        ] {
            if active {
                *counts.entry(label.to_string()).or_insert(0) += 1;
            }
        }
    }
    counts
}

fn invalid(detail: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, detail.into()).into()
}
