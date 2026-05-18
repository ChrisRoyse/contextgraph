use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use context_graph_mejepa::{
    mejepa_mincut_panel, open_mincut_rocksdb, pairwise_mi_corpus_hash, read_pairwise_mi_matrix,
    write_mincut_report_sync_readback, write_pairwise_mi_matrix_sync_readback, MincutOptions,
    PairwiseMiMatrixHealth, PanelGraphSource, PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
};
use context_graph_mejepa_cf::{CF_MEJEPA_MINCUT_REPORTS, CF_MEJEPA_PAIRWISE_MI};
use context_graph_mejepa_train::learning_signal::{compute_pairwise_mi_matrix, PairwiseMiAuditor};
use serde::{Deserialize, Serialize};
use serde_json::json;

const TASK: &str = "TASK-PY-G-007";
const ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-007-pairwise-mi-fsv";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SyntheticKnownMiEvidence {
    clone_pair: String,
    clone_mi: f32,
    clone_within_tolerance: bool,
    independent_pair: String,
    independent_mi: f32,
    independent_within_tolerance: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FsvReport {
    task: &'static str,
    passed: bool,
    run_root: PathBuf,
    canonical_path: PathBuf,
    audit_csv_path: PathBuf,
    db_path: PathBuf,
    corpus_shard_hash: String,
    series_source: serde_json::Value,
    slot_count: usize,
    sample_count: usize,
    expected_pair_count: usize,
    rows_written: usize,
    byte_identical_readback: bool,
    matrix_readback_equal: bool,
    rows_after_reopen: usize,
    synthetic_known_mi: Option<SyntheticKnownMiEvidence>,
    health: PairwiseMiMatrixHealth,
    mincut_report_id: String,
    mincut_graph_source_kind: String,
    mincut_source_row_count: usize,
    mincut_cut_value: f32,
    source_of_truth: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExternalSeriesInput {
    series_by_slot: BTreeMap<String, Vec<f32>>,
    #[serde(default)]
    corpus_shard_hash: Option<String>,
    #[serde(default)]
    source: serde_json::Value,
}

#[derive(Debug)]
struct SeriesInput {
    series_by_slot: BTreeMap<String, Vec<f32>>,
    corpus_shard_hash: Option<String>,
    source: serde_json::Value,
    require_synthetic_known_mi: bool,
}

fn main() -> anyhow::Result<()> {
    let run_id = now_unix_ms()?;
    let root = PathBuf::from(ROOT);
    let run_root = root.join(format!("run-{run_id}"));
    let audit_dir = run_root.join("audit");
    let db_path = run_root.join("rocksdb");
    fs::create_dir_all(&audit_dir)?;

    let series_input = load_series_input()?;
    let matrix = compute_pairwise_mi_matrix(&series_input.series_by_slot)?;
    let auditor = PairwiseMiAuditor::new(1, &audit_dir)?;
    let summary = auditor.run_from_slot_series(&series_input.series_by_slot, run_id as u64)?;
    if summary.matrix != matrix {
        anyhow::bail!("auditor matrix differs from direct compute_pairwise_mi_matrix output");
    }

    let corpus_shard_hash = match series_input.corpus_shard_hash {
        Some(hash) => hash,
        None => pairwise_mi_corpus_hash(&matrix.slots, &matrix.values)?,
    };
    let created_at_unix_ms = run_id;
    let expected_pair_count = matrix.slots.len() * (matrix.slots.len() - 1) / 2;
    if series_input.require_synthetic_known_mi && expected_pair_count != 78 {
        anyhow::bail!("expected 13-slot panel to produce 78 pairs; got {expected_pair_count}");
    }

    let db = open_mincut_rocksdb(&db_path)?;
    let write = write_pairwise_mi_matrix_sync_readback(
        db.as_ref(),
        &corpus_shard_hash,
        &matrix.slots,
        &matrix.values,
        run_id as u64,
        summary.sample_count,
        created_at_unix_ms,
        PAIRWISE_MI_ESTIMATOR_PARTITIONED_NMI,
    )?;
    drop(db);

    let reopened = open_mincut_rocksdb(&db_path)?;
    let readback = read_pairwise_mi_matrix(
        reopened.as_ref(),
        Some(&corpus_shard_hash),
        Some(created_at_unix_ms),
        1_000,
    )?;
    let matrix_readback_equal = readback.slots == matrix.slots && readback.values == matrix.values;
    if !matrix_readback_equal {
        anyhow::bail!("CF_MEJEPA_PAIRWISE_MI reopen matrix readback mismatch");
    }

    let known = if series_input.require_synthetic_known_mi {
        Some(known_mi_evidence(&matrix)?)
    } else {
        None
    };
    let mincut = mejepa_mincut_panel(
        Some(reopened.as_ref()),
        PanelGraphSource::PairwiseMiMatrix {
            corpus_shard_hash: Some(corpus_shard_hash.clone()),
            created_at_unix_ms: Some(created_at_unix_ms),
            max_rows: 1_000,
        },
        MincutOptions::default(),
        created_at_unix_ms + 1,
    )?;
    write_mincut_report_sync_readback(reopened.as_ref(), &mincut)?;

    let passed = write.rows_written == expected_pair_count
        && write.byte_identical_readback
        && write.matrix_readback_equal
        && matrix_readback_equal
        && readback.source_row_count == expected_pair_count
        && known
            .as_ref()
            .map(|known| known.clone_within_tolerance && known.independent_within_tolerance)
            .unwrap_or(true)
        && mincut.graph_source_kind == "pairwise_mi_matrix"
        && mincut.source_row_count == expected_pair_count
        && mincut.source_of_truth_cf == CF_MEJEPA_MINCUT_REPORTS;

    let canonical_path = root.join("pairwise_mi_fsv.json");
    let report = FsvReport {
        task: TASK,
        passed,
        run_root: run_root.clone(),
        canonical_path: canonical_path.clone(),
        audit_csv_path: summary.path.clone(),
        db_path: db_path.clone(),
        corpus_shard_hash,
        series_source: series_input.source,
        slot_count: matrix.slots.len(),
        sample_count: summary.sample_count,
        expected_pair_count,
        rows_written: write.rows_written,
        byte_identical_readback: write.byte_identical_readback,
        matrix_readback_equal: write.matrix_readback_equal && matrix_readback_equal,
        rows_after_reopen: readback.source_row_count,
        synthetic_known_mi: known,
        health: readback.health,
        mincut_report_id: mincut.report_id,
        mincut_graph_source_kind: mincut.graph_source_kind,
        mincut_source_row_count: mincut.source_row_count,
        mincut_cut_value: mincut.cut_value,
        source_of_truth: json!({
            "pairwiseMiCf": CF_MEJEPA_PAIRWISE_MI,
            "mincutReportCf": CF_MEJEPA_MINCUT_REPORTS,
            "csvReadbackPath": summary.readback_path,
            "dbReopened": true,
            "innerLlmInvoked": false
        }),
    };
    write_json_sync(run_root.join("pairwise_mi_fsv.json"), &report)?;
    write_json_sync(&canonical_path, &report)?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !passed {
        anyhow::bail!("TASK-PY-G-007 FSV failed");
    }
    Ok(())
}

fn load_series_input() -> anyhow::Result<SeriesInput> {
    match std::env::var_os("TASK_PY_G_007_SERIES_JSON") {
        Some(path) => {
            let path = PathBuf::from(path);
            let input: ExternalSeriesInput = serde_json::from_slice(&fs::read(&path)?)?;
            Ok(SeriesInput {
                series_by_slot: input.series_by_slot,
                corpus_shard_hash: input.corpus_shard_hash,
                source: json!({
                    "mode": "external_series_json",
                    "path": path,
                    "details": input.source
                }),
                require_synthetic_known_mi: false,
            })
        }
        None => Ok(SeriesInput {
            series_by_slot: synthetic_13_slot_series(),
            corpus_shard_hash: None,
            source: json!({
                "mode": "synthetic_13_slot_known_mi",
                "expectedPairCount": 78
            }),
            require_synthetic_known_mi: true,
        }),
    }
}

fn synthetic_13_slot_series() -> BTreeMap<String, Vec<f32>> {
    let mut out = BTreeMap::new();
    let samples = 1_000usize;
    let mut make = |name: &str, f: fn(usize, usize, usize) -> f32| {
        let mut values = Vec::with_capacity(samples);
        for idx in 0..samples {
            let x = idx % 10;
            let y = (idx / 10) % 10;
            let z = (idx / 100) % 10;
            values.push(f(x, y, z));
        }
        out.insert(name.to_string(), values);
    };
    make("e00_axis_x", |x, _, _| scale10(x));
    make("e01_axis_x_clone", |x, _, _| scale10(x));
    make("e02_axis_y", |_, y, _| scale10(y));
    make("e03_axis_z", |_, _, z| scale10(z));
    make("e04_xy_sum", |x, y, _| scale19(x + y));
    make("e05_xz_sum", |x, _, z| scale19(x + z));
    make("e06_yz_sum", |_, y, z| scale19(y + z));
    make("e07_xyz_sum", |x, y, z| scale28(x + y + z));
    make("e08_x_bucket", |x, _, _| if x < 5 { 0.0 } else { 1.0 });
    make("e09_y_bucket", |_, y, _| if y < 5 { 0.0 } else { 1.0 });
    make("e10_z_bucket", |_, _, z| if z < 5 { 0.0 } else { 1.0 });
    make("e11_x_y_contrast", |x, y, _| scale19((9 + x) - y));
    make("e12_y_z_contrast", |_, y, z| scale19((9 + y) - z));
    out
}

fn known_mi_evidence(
    matrix: &context_graph_mejepa_train::learning_signal::PairwiseMiMatrix,
) -> anyhow::Result<SyntheticKnownMiEvidence> {
    let clone_mi = mi_value(matrix, "e00_axis_x", "e01_axis_x_clone")?;
    let independent_mi = mi_value(matrix, "e00_axis_x", "e02_axis_y")?;
    Ok(SyntheticKnownMiEvidence {
        clone_pair: "e00_axis_x::e01_axis_x_clone".to_string(),
        clone_mi,
        clone_within_tolerance: clone_mi >= 0.99,
        independent_pair: "e00_axis_x::e02_axis_y".to_string(),
        independent_mi,
        independent_within_tolerance: independent_mi <= 0.05,
    })
}

fn mi_value(
    matrix: &context_graph_mejepa_train::learning_signal::PairwiseMiMatrix,
    left: &str,
    right: &str,
) -> anyhow::Result<f32> {
    let left_idx = matrix
        .slots
        .iter()
        .position(|slot| slot == left)
        .ok_or_else(|| anyhow::anyhow!("missing slot {left}"))?;
    let right_idx = matrix
        .slots
        .iter()
        .position(|slot| slot == right)
        .ok_or_else(|| anyhow::anyhow!("missing slot {right}"))?;
    Ok(matrix.values[left_idx][right_idx])
}

fn scale10(value: usize) -> f32 {
    value as f32 / 9.0
}

fn scale19(value: usize) -> f32 {
    value as f32 / 18.0
}

fn scale28(value: usize) -> f32 {
    value as f32 / 27.0
}

fn now_unix_ms() -> anyhow::Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64)
}

fn write_json_sync(path: impl AsRef<Path>, value: &impl Serialize) -> anyhow::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path.as_ref())?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
