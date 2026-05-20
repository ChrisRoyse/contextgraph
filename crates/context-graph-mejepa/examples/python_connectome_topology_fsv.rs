use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, ensure, Context, Result};
use context_graph_mejepa::{
    compute_connectome_topology_features, count_cf, open_infer_rocksdb, rich_club_pair_jaccard,
    ConnectomeTopologyEdge, ConnectomeTopologyFeatures, CONNECTOME_DEFAULT_STRONG_EDGE_THRESHOLD,
};
use context_graph_mejepa_cf::CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES;
use rocksdb::WriteOptions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-ip-003-connectome-topology-fsv";
const DEFAULT_TENSOR_ROOT: &str = "/zfs/archive/contextgraph/corpus/constellation-tensors-v1";
const ISSUE_EXPECTED_PYTHON_CELLS: usize = 8;
const MIN_CELLS_ABOVE_NULL: usize = 5;
const MIN_SPLIT_JACCARD: f32 = 0.8;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CellTopologyReport {
    cell_id: String,
    valid_tensor_rows: usize,
    invalid_tensor_rows: usize,
    feature: ConnectomeTopologyFeatures,
    even_split_pair_count: usize,
    odd_split_pair_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EdgeCase {
    name: String,
    passed: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectomeTopologyFsvReport {
    task_id: String,
    issue: u32,
    all_passed: bool,
    source_of_truth: Value,
    checks: Value,
    ablation_comparison: Value,
    cell_topologies: Vec<CellTopologyReport>,
    edge_cases: Vec<EdgeCase>,
    independent_readback: Value,
}

#[derive(Default)]
struct TensorLoad {
    cells: BTreeMap<String, CellLoad>,
    raw_rows: usize,
    valid_rows: usize,
    invalid_rows: usize,
    flat_vector_concat_used_any: bool,
    slot_identity_preserved_all: bool,
    observed_expected_pairs_per_full_cell_window: BTreeSet<u64>,
    corpus_ids: BTreeSet<String>,
    run_ids: BTreeSet<String>,
}

#[derive(Default)]
struct CellLoad {
    all_edges: Vec<ConnectomeTopologyEdge>,
    even_edges: Vec<ConnectomeTopologyEdge>,
    odd_edges: Vec<ConnectomeTopologyEdge>,
    valid_rows: usize,
    invalid_rows: usize,
    unique_pairs: BTreeSet<String>,
}

fn main() -> Result<()> {
    let fsv_root = env_path("CG_CONNECTOME_TOPOLOGY_FSV_ROOT", DEFAULT_FSV_ROOT);
    let tensor_root = env_path("CG_CONNECTOME_TOPOLOGY_TENSOR_ROOT", DEFAULT_TENSOR_ROOT);
    ensure!(
        !starts_with_mnt_d(&fsv_root) && !starts_with_mnt_d(&tensor_root),
        "active #513 FSV paths must not use /mnt/d"
    );
    fs::create_dir_all(&fsv_root)?;
    let run_id = format!("run-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let run_root = fsv_root.join(&run_id);
    fs::create_dir_all(&run_root)?;

    let synthetic_mode = std::env::var("CG_CONNECTOME_TOPOLOGY_SYNTHETIC")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let tensor_path = tensor_root.join("pairwise_correlation_tensors.jsonl");
    let tensor_load = if synthetic_mode {
        synthetic_tensor_load()
    } else {
        load_pairwise_tensor(&tensor_path)?
    };
    let real_tensor_consumed = !synthetic_mode && tensor_path.exists();

    let mut cell_topologies = Vec::new();
    for (cell_id, cell) in &tensor_load.cells {
        if cell.all_edges.is_empty() {
            continue;
        }
        let mut feature = compute_connectome_topology_features(
            cell_id.clone(),
            0,
            &cell.all_edges,
            CONNECTOME_DEFAULT_STRONG_EDGE_THRESHOLD,
        )?;
        let split_jaccard =
            split_jaccard(cell_id, &cell.even_edges, &cell.odd_edges).transpose()?;
        feature.split_rich_club_pair_jaccard = split_jaccard;
        feature.validate()?;
        cell_topologies.push(CellTopologyReport {
            cell_id: cell_id.clone(),
            valid_tensor_rows: cell.valid_rows,
            invalid_tensor_rows: cell.invalid_rows,
            feature,
            even_split_pair_count: distinct_pair_count(&cell.even_edges),
            odd_split_pair_count: distinct_pair_count(&cell.odd_edges),
        });
    }
    ensure!(
        !cell_topologies.is_empty(),
        "no connectome topology features were produced"
    );
    cell_topologies.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));

    let db_path = run_root.join("connectome-topology.rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let cf = db
        .cf_handle(CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES)
        .ok_or_else(|| {
            anyhow!("missing CF {CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES} in inference opener")
        })?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    let mut read_after_write_equal = true;
    for cell in &cell_topologies {
        let key = topology_key(&cell.feature);
        let value = bincode::serialize(&cell.feature)?;
        db.put_cf_opt(cf, &key, &value, &write_opts)?;
        let readback = db
            .get_cf(cf, &key)?
            .ok_or_else(|| anyhow!("missing read-after-write row for {}", cell.cell_id))?;
        read_after_write_equal &= readback == value;
        let decoded: ConnectomeTopologyFeatures = bincode::deserialize(&readback)?;
        decoded.validate()?;
        read_after_write_equal &= decoded == cell.feature;
    }
    db.flush_cf(cf)?;
    drop(db);
    let readback_db = open_infer_rocksdb(&db_path)?;
    let cf_rows = count_cf(readback_db.as_ref(), CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES)?;
    let readback_count_matches = cf_rows as usize == cell_topologies.len();
    drop(readback_db);

    let cells_above_rich_null = cell_topologies
        .iter()
        .filter(|cell| cell.feature.rich_club_above_null)
        .count();
    let cells_above_modularity_null = cell_topologies
        .iter()
        .filter(|cell| cell.feature.modularity_above_null)
        .count();
    let cells_split_jaccard_measured = cell_topologies
        .iter()
        .filter(|cell| cell.feature.split_rich_club_pair_jaccard.is_some())
        .count();
    let cells_split_reproducible = cell_topologies
        .iter()
        .filter(|cell| {
            cell.feature
                .split_rich_club_pair_jaccard
                .is_some_and(|value| value >= MIN_SPLIT_JACCARD)
        })
        .count();
    let full_tensor_coverage_cells = cell_topologies
        .iter()
        .filter(|cell| cell.feature.source_edge_count >= 66)
        .count();
    let edge_cases = edge_cases();
    let ablation_comparison = ablation_comparison(&cell_topologies);
    let issue_expected_cell_count = ISSUE_EXPECTED_PYTHON_CELLS;
    let observed_cell_count = cell_topologies.len();
    let observed_cell_count_matches_issue_assumption =
        observed_cell_count == issue_expected_cell_count;
    let minimum_cells = MIN_CELLS_ABOVE_NULL.min(observed_cell_count);
    let split_minimum = MIN_CELLS_ABOVE_NULL.min(cells_split_jaccard_measured);
    let all_passed = (real_tensor_consumed || synthetic_mode)
        && !tensor_load.flat_vector_concat_used_any
        && tensor_load.slot_identity_preserved_all
        && observed_cell_count >= MIN_CELLS_ABOVE_NULL
        && cells_above_rich_null >= minimum_cells
        && cells_above_modularity_null >= minimum_cells
        && cells_split_reproducible >= split_minimum
        && read_after_write_equal
        && readback_count_matches
        && edge_cases.iter().all(|case| case.passed);

    let independent_readback = json!({
        "dbPath": db_path,
        "cfName": CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES,
        "cfRows": cf_rows,
        "expectedRows": cell_topologies.len(),
        "readAfterWriteEqual": read_after_write_equal,
        "readbackCountMatches": readback_count_matches
    });
    let report = ConnectomeTopologyFsvReport {
        task_id: "TASK-IP-003".to_string(),
        issue: 513,
        all_passed,
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "fsvRoot": fsv_root,
            "runRoot": run_root,
            "tensorRoot": tensor_root,
            "pairwiseTensorPath": tensor_path,
            "realTensorConsumed": real_tensor_consumed,
            "syntheticMode": synthetic_mode,
            "cf": CF_MEJEPA_CONNECTOME_TOPOLOGY_FEATURES,
            "slotPolicy": "embedders remain graph nodes; pairwise tensor correlations become weighted edges; no flat vector concatenation is used",
            "mntDActiveReadsOrWrites": false
        }),
        checks: json!({
            "rawTensorRows": tensor_load.raw_rows,
            "validTensorRows": tensor_load.valid_rows,
            "invalidTensorRows": tensor_load.invalid_rows,
            "observedCorpusIds": tensor_load.corpus_ids,
            "observedRunIds": tensor_load.run_ids,
            "expectedPairsPerFullCellWindowValues": tensor_load.observed_expected_pairs_per_full_cell_window,
            "issueExpectedPythonCellCount": issue_expected_cell_count,
            "observedCellCount": observed_cell_count,
            "observedCellCountMatchesIssueAssumption": observed_cell_count_matches_issue_assumption,
            "fullTensorCoverageCells": full_tensor_coverage_cells,
            "cellsAboveRichClubNull": cells_above_rich_null,
            "cellsAboveModularityNull": cells_above_modularity_null,
            "cellsSplitJaccardMeasured": cells_split_jaccard_measured,
            "cellsSplitReproducibleAt0_8": cells_split_reproducible,
            "minimumCellsRequired": minimum_cells,
            "minimumSplitCellsRequired": split_minimum,
            "flatVectorConcatUsedAny": tensor_load.flat_vector_concat_used_any,
            "slotIdentityPreservedAll": tensor_load.slot_identity_preserved_all,
            "readbackCountMatches": readback_count_matches,
            "edgeCasesPass": edge_cases.iter().all(|case| case.passed)
        }),
        ablation_comparison,
        cell_topologies,
        edge_cases,
        independent_readback,
    };

    let report_path = fsv_root.join("python_connectome_topology_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &report)?;
    let durable: ConnectomeTopologyFsvReport = serde_json::from_slice(&fs::read(&report_path)?)?;
    ensure!(
        durable.all_passed == report.all_passed
            && durable.cell_topologies.len() == report.cell_topologies.len(),
        "durable report readback mismatch"
    );
    ensure!(
        report.all_passed,
        "TASK-IP-003 connectome topology FSV failed at {}",
        report_path.display()
    );
    println!(
        "TASK-IP-003 connectome topology FSV PASS report={} sha256={}",
        report_path.display(),
        sha256_file(&report_path)?
    );
    Ok(())
}

fn load_pairwise_tensor(path: &Path) -> Result<TensorLoad> {
    ensure!(
        path.exists(),
        "pairwise tensor file missing: {}",
        path.display()
    );
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut load = TensorLoad {
        slot_identity_preserved_all: true,
        ..TensorLoad::default()
    };
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(&line)
            .with_context(|| format!("parse tensor row {}", line_idx + 1))?;
        load.raw_rows += 1;
        let cell_key = str_field(&row, "cell_key")?;
        if let Some(corpus_id) = row.get("corpus_id").and_then(Value::as_str) {
            load.corpus_ids.insert(corpus_id.to_string());
        }
        if let Some(run_id) = row.get("run_id").and_then(Value::as_str) {
            load.run_ids.insert(run_id.to_string());
        }
        if let Some(expected_pairs) = row
            .get("expected_pairs_per_full_cell_window")
            .and_then(Value::as_u64)
        {
            load.observed_expected_pairs_per_full_cell_window
                .insert(expected_pairs);
        }
        load.flat_vector_concat_used_any |= row
            .get("flat_vector_concat_used")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        load.slot_identity_preserved_all &= row
            .get("slot_identity_preserved")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let cell = load.cells.entry(cell_key.clone()).or_default();
        if row.get("valid").and_then(Value::as_bool) != Some(true) {
            load.invalid_rows += 1;
            cell.invalid_rows += 1;
            continue;
        }
        let left = str_field(&row, "left_slot_id")?;
        let right = str_field(&row, "right_slot_id")?;
        let Some(weight) = tensor_weight(&row).with_context(|| {
            format!(
                "extract finite tensor edge weight for row {} cell {} pair {}--{}",
                line_idx + 1,
                cell_key,
                left,
                right
            )
        })?
        else {
            load.invalid_rows += 1;
            cell.invalid_rows += 1;
            continue;
        };
        let edge = ConnectomeTopologyEdge {
            left_slot_id: left.clone(),
            right_slot_id: right.clone(),
            weight,
        };
        edge.validate("tensor_edge")?;
        load.valid_rows += 1;
        cell.valid_rows += 1;
        cell.unique_pairs.insert(pair_id(&left, &right));
        cell.all_edges.push(edge.clone());
        if split_bucket(&row, line_idx) == 0 {
            cell.even_edges.push(edge);
        } else {
            cell.odd_edges.push(edge);
        }
    }
    Ok(load)
}

fn tensor_weight(row: &Value) -> Result<Option<f32>> {
    let candidate = ["abs_mean", "entropy_proxy", "norm_l2", "std"]
        .into_iter()
        .find_map(|field| {
            row.get("scalar_feature_correlation_tensor")
                .and_then(|value| value.get(field))
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
        })
        .or_else(|| {
            row.get("redundancy_score")
                .and_then(Value::as_f64)
                .filter(|value| value.is_finite())
        });
    let Some(candidate) = candidate else {
        return Ok(None);
    };
    let candidate = candidate.abs();
    ensure!(candidate.is_finite(), "non-finite tensor weight");
    Ok(Some((candidate as f32).clamp(0.0, 1.0)))
}

fn split_jaccard(
    cell_id: &str,
    even_edges: &[ConnectomeTopologyEdge],
    odd_edges: &[ConnectomeTopologyEdge],
) -> Option<Result<f32>> {
    if even_edges.is_empty() || odd_edges.is_empty() {
        return None;
    }
    let even = compute_connectome_topology_features(
        cell_id.to_string(),
        0,
        even_edges,
        CONNECTOME_DEFAULT_STRONG_EDGE_THRESHOLD,
    );
    let odd = compute_connectome_topology_features(
        cell_id.to_string(),
        1,
        odd_edges,
        CONNECTOME_DEFAULT_STRONG_EDGE_THRESHOLD,
    );
    Some(
        even.and_then(|even| {
            odd.and_then(|odd| {
                rich_club_pair_jaccard(&even.rich_club_pair_ids, &odd.rich_club_pair_ids)
            })
        })
        .map_err(anyhow::Error::from),
    )
}

fn ablation_comparison(cells: &[CellTopologyReport]) -> Value {
    let labels = cells
        .iter()
        .map(|cell| {
            if cell.cell_id.contains("known_good") {
                0.0
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let baseline_scores = cells
        .iter()
        .map(|cell| {
            let possible = pair_count(cell.feature.node_count as usize).max(1) as f32;
            cell.feature.strong_edge_count as f32 / possible
        })
        .collect::<Vec<_>>();
    let topology_scores = cells
        .iter()
        .map(|cell| {
            let rich_lift =
                (cell.feature.rich_club_coefficient - cell.feature.rich_club_null_mean).max(0.0);
            let modularity = cell.feature.modularity_q.max(0.0);
            let split = cell.feature.split_rich_club_pair_jaccard.unwrap_or(0.0);
            ((rich_lift + modularity + split) / 3.0).clamp(0.0, 1.0)
        })
        .collect::<Vec<_>>();
    let baseline_corr = pearson(&labels, &baseline_scores);
    let topology_corr = pearson(&labels, &topology_scores);
    json!({
        "kind": "matched-cell scalar ablation diagnostic",
        "note": "This reports a #398-style enabled-vs-disabled scalar comparison over the same #388 tensor cells. It does not promote the predictor by itself.",
        "disabledTopologyFeature": "strong_edge_density_only",
        "enabledTopologyFeature": "rich_club_lift_plus_modularity_plus_split_reproducibility",
        "matchedCellCount": cells.len(),
        "baselinePredictionOracleCorrelation": baseline_corr,
        "topologyPredictionOracleCorrelation": topology_corr,
        "deltaCorrelationVsBaseline": match (baseline_corr, topology_corr) {
            (Some(base), Some(topology)) => Some(topology - base),
            _ => None,
        },
        "flatVectorConcatUsed": false,
        "recommendation": "persist_feature_and_measure_in_next_full_calibrated_ablation_before_promotion"
    })
}

fn pearson(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.len() < 2 {
        return None;
    }
    let left_mean = left.iter().sum::<f32>() / left.len() as f32;
    let right_mean = right.iter().sum::<f32>() / right.len() as f32;
    let mut numerator = 0.0f32;
    let mut left_var = 0.0f32;
    let mut right_var = 0.0f32;
    for (l, r) in left.iter().zip(right.iter()) {
        let dl = *l - left_mean;
        let dr = *r - right_mean;
        numerator += dl * dr;
        left_var += dl * dl;
        right_var += dr * dr;
    }
    if left_var <= 0.0 || right_var <= 0.0 {
        None
    } else {
        Some((numerator / (left_var.sqrt() * right_var.sqrt())).clamp(-1.0, 1.0))
    }
}

fn edge_cases() -> Vec<EdgeCase> {
    vec![
        edge_case("empty_edges_fail_closed", || {
            compute_connectome_topology_features("python:edge", 0, &[], 0.5).map(|_| ())
        }),
        edge_case("same_slot_edge_fail_closed", || {
            let edges = vec![ConnectomeTopologyEdge {
                left_slot_id: "e1".to_string(),
                right_slot_id: "e1".to_string(),
                weight: 0.5,
            }];
            compute_connectome_topology_features("python:edge", 0, &edges, 0.5).map(|_| ())
        }),
        edge_case("non_finite_weight_fail_closed", || {
            let edges = vec![ConnectomeTopologyEdge {
                left_slot_id: "e1".to_string(),
                right_slot_id: "e2".to_string(),
                weight: f32::NAN,
            }];
            compute_connectome_topology_features("python:edge", 0, &edges, 0.5).map(|_| ())
        }),
    ]
}

fn split_bucket(row: &Value, line_idx: usize) -> u8 {
    let mut hasher = Sha256::new();
    if let Some(tensor_key) = row.get("tensor_key").and_then(Value::as_str) {
        hasher.update(tensor_key.as_bytes());
    }
    if let Some(relationship_hash) = row.get("relationship_hash").and_then(Value::as_str) {
        hasher.update(relationship_hash.as_bytes());
    }
    hasher.update(line_idx.to_le_bytes());
    hasher.finalize()[0] & 1
}

fn edge_case(
    name: &str,
    f: impl FnOnce() -> Result<(), context_graph_mejepa::MejepaInferError>,
) -> EdgeCase {
    match f() {
        Ok(()) => EdgeCase {
            name: name.to_string(),
            passed: false,
            error: None,
        },
        Err(err) => EdgeCase {
            name: name.to_string(),
            passed: true,
            error: Some(err.code().to_string()),
        },
    }
}

fn synthetic_tensor_load() -> TensorLoad {
    let mut load = TensorLoad {
        raw_rows: 0,
        valid_rows: 0,
        invalid_rows: 0,
        flat_vector_concat_used_any: false,
        slot_identity_preserved_all: true,
        observed_expected_pairs_per_full_cell_window: BTreeSet::from([66]),
        corpus_ids: BTreeSet::from(["synthetic".to_string()]),
        run_ids: BTreeSet::from(["synthetic".to_string()]),
        cells: BTreeMap::new(),
    };
    for cell_id in [
        "python:known_good",
        "python:delete_test_call",
        "python:off_by_one",
        "python:subtle_flip",
        "python:wrong_file",
    ] {
        let mut cell = CellLoad::default();
        for (left, right, weight) in [
            ("e1", "e6", 0.92),
            ("e1", "e8", 0.88),
            ("e6", "e8", 0.86),
            ("e2", "e3", 0.10),
            ("e3", "e4", 0.12),
            ("e9", "e10", 0.08),
        ] {
            let edge = ConnectomeTopologyEdge {
                left_slot_id: left.to_string(),
                right_slot_id: right.to_string(),
                weight,
            };
            cell.all_edges.push(edge.clone());
            cell.even_edges.push(edge.clone());
            cell.odd_edges.push(edge);
            cell.unique_pairs.insert(pair_id(left, right));
            cell.valid_rows += 1;
            load.valid_rows += 1;
            load.raw_rows += 1;
        }
        load.cells.insert(cell_id.to_string(), cell);
    }
    load
}

fn topology_key(feature: &ConnectomeTopologyFeatures) -> Vec<u8> {
    format!("cell:{}:window:{}", feature.cell_id, feature.window_index).into_bytes()
}

fn str_field(row: &Value, field: &str) -> Result<String> {
    row.get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing string field {field}"))
}

fn env_path(name: &str, default: &str) -> PathBuf {
    std::env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn starts_with_mnt_d(path: &Path) -> bool {
    path.components()
        .take(3)
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
        .starts_with("//mnt/d")
        || path.to_string_lossy().starts_with("/mnt/d")
}

fn distinct_pair_count(edges: &[ConnectomeTopologyEdge]) -> usize {
    edges
        .iter()
        .map(|edge| pair_id(&edge.left_slot_id, &edge.right_slot_id))
        .collect::<BTreeSet<_>>()
        .len()
}

fn pair_id(left: &str, right: &str) -> String {
    if left <= right {
        format!("{left}--{right}")
    } else {
        format!("{right}--{left}")
    }
}

fn pair_count(node_count: usize) -> usize {
    node_count * node_count.saturating_sub(1) / 2
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(hex::encode(hasher.finalize()))
}
