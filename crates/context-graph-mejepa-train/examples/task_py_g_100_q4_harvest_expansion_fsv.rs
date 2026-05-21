// FROZEN 2026-05-19 per #408 — forensic continuity only; do not re-run, do not modify.
// Q4 ambiguous surface frozen as display-only. See docs/futurebuild/01_overview.md §1.1.
use context_graph_mejepa::q4_trust_gate::{
    default_q4_requirements, evaluate_q4_trust_gate, q4_trust_gate_report_from_catalog_path,
    Q4EvidenceCatalog, Q4HeadKind, Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS,
};
use context_graph_mejepa_instruments::{
    Q4AccuracyLabelStore, Q4AccuracySignalRecord, Q4CostKind, Q4CostLabelStore, Q4CostSignalRecord,
    Q4PerfLabelStore, Q4PerfSignalRecord, Q4_CONFIDENCE_MIN_LABELS,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const FSV_ROOT: &str = "/zfs/archive/contextgraph/fsv/task-py-g-100-q4-harvest-expansion-fsv";
const CATALOG_PATH: &str = "/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json";
const PERF_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-100-q4-perf-expanded-harvest-fsv/q4_perf_expanded_harvest_fsv.json";
const ACCURACY_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-100-q4-accuracy-expanded-harvest-fsv/q4_accuracy_expanded_harvest_fsv.json";
const COST_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-100-q4-cost-expanded-harvest-fsv/q4_cost_expanded_harvest_fsv.json";
const CALIBRATION_FSV: &str = "/zfs/archive/contextgraph/fsv/task-py-g-012-q4-confidence-calibration-fsv/q4_confidence_calibration_fsv.json";

#[derive(Debug, Serialize)]
struct HeadHarvest {
    head: &'static str,
    fsv_path: PathBuf,
    sha256: String,
    producer_rows: u64,
    rocksdb_path: Option<String>,
    cf: Option<String>,
    direct_readback: Value,
}

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

fn main() -> FsvResult<()> {
    let fsv_root = env_path("CG_Q4_HARVEST_EXPANSION_FSV_ROOT", FSV_ROOT);
    let catalog_path = env_path("CG_Q4_HARVEST_EXPANSION_CATALOG_PATH", CATALOG_PATH);
    fs::create_dir_all(&fsv_root)?;
    if let Some(parent) = catalog_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let perf_fsv = env_path("CG_Q4_HARVEST_EXPANSION_PERF_FSV", PERF_FSV);
    let accuracy_fsv = env_path("CG_Q4_HARVEST_EXPANSION_ACCURACY_FSV", ACCURACY_FSV);
    let cost_fsv = env_path("CG_Q4_HARVEST_EXPANSION_COST_FSV", COST_FSV);
    let calibration_fsv = env_path("CG_Q4_HARVEST_EXPANSION_CALIBRATION_FSV", CALIBRATION_FSV);
    let evidence_path = fsv_root.join("q4_harvest_expansion_fsv.json");

    let harvests = vec![
        read_head_harvest("perf", &perf_fsv)?,
        read_head_harvest("accuracy", &accuracy_fsv)?,
        read_head_harvest("cost", &cost_fsv)?,
    ];
    for harvest in &harvests {
        require(
            &format!("{} producer floor met", harvest.head),
            harvest.producer_rows >= Q4_CONFIDENCE_MIN_LABELS as u64,
        )?;
    }

    let calibration_doc: Value = serde_json::from_slice(&fs::read(&calibration_fsv)?)?;
    require(
        "calibration FSV all_passed",
        calibration_doc["all_passed"].as_bool() == Some(true),
    )?;
    let calibration_reports = calibration_doc["happy_path"]["after_state"]["calibration_reports"]
        .as_array()
        .ok_or("missing calibration reports")?;
    for head in ["perf", "accuracy", "cost"] {
        let report = calibration_reports
            .iter()
            .find(|report| report["head"].as_str() == Some(head))
            .ok_or_else(|| format!("missing calibration report for {head}"))?;
        require(
            &format!("{head} calibration uses the producer floor"),
            report["label_count"].as_u64().unwrap_or(0) >= Q4_CONFIDENCE_MIN_LABELS as u64,
        )?;
        require(
            &format!("{head} calibration is trust-supported"),
            matches!(
                report["status"].as_str(),
                Some("calibrated" | "temperature_fallback")
            ),
        )?;
    }

    let catalog: Q4EvidenceCatalog = serde_json::from_slice(&fs::read(&catalog_path)?)?;
    let trust_report = q4_trust_gate_report_from_catalog_path(&catalog_path)?;
    require(
        "Q4 trust decision is eligible after producer expansion",
        trust_report.q4_trust_decision_eligible,
    )?;
    require(
        "every aggregate Q4 head is ready",
        trust_report.heads.iter().all(|head| head.q4_head_ready),
    )?;
    require("slot evidence is exact", active_slot_set_is_exact(&catalog))?;

    let edge_cases = vec![
        edge_missing_perf_fails_closed(&catalog)?,
        edge_below_floor_fails_closed(&catalog)?,
        edge_wrong_slots_fail_closed(&catalog)?,
        edge_structural_heads_excluded()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "Q4 harvest expansion edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let evidence = json!({
        "task": "TASK-PY-G-100",
        "issue": 401,
        "all_passed": true,
        "source_of_truth": {
            "fsv_root": fsv_root,
            "evidence_path": evidence_path,
            "q4_catalog_path": catalog_path,
            "perf_fsv": perf_fsv,
            "accuracy_fsv": accuracy_fsv,
            "cost_fsv": cost_fsv,
            "calibration_fsv": calibration_fsv,
        },
        "happy_path": {
            "trigger": "Expand perf/accuracy/cost Q4 producer harvests on aiwonder, rerun Q4 confidence calibration, and read the shared Q4 trust catalog.",
            "expected": {
                "producer_rows_min": Q4_CONFIDENCE_MIN_LABELS,
                "calibration_rows_min": Q4_CONFIDENCE_MIN_LABELS,
                "q4_trust_decision_eligible": true,
                "aggregate_heads": aggregate_head_names(),
            },
            "after_state": {
                "producer_harvests": harvests,
                "calibration_fsv_sha256": sha256_file(&calibration_fsv)?,
                "catalog_sha256": sha256_file(&catalog_path)?,
                "catalog": catalog,
                "q4_trust_gate": trust_report,
            }
        },
        "edge_cases": edge_cases,
        "implementation_notes": {
            "slot_policy": "The Q4 catalog cites per-slot evidence and exact active embedder slots; no flat vector concatenation can satisfy this FSV.",
            "unsupported_head_policy": "Security and structural heads remain excluded from aggregate Q4 eligibility until producer and calibration evidence exists.",
            "legacy_storage_policy": "All default paths are aiwonder /zfs/archive/contextgraph roots."
        }
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn read_head_harvest(head: &'static str, path: &Path) -> FsvResult<HeadHarvest> {
    let doc: Value = serde_json::from_slice(&fs::read(path)?)?;
    require(
        &format!("{head} producer FSV all_passed"),
        doc["all_passed"].as_bool() == Some(true),
    )?;
    let rocksdb_path = doc["source_of_truth"]["rocksdb_path"]
        .as_str()
        .ok_or_else(|| format!("missing RocksDB path for {head}"))?;
    let label = doc["happy_path"]["after_state"]["labels"]
        .as_array()
        .and_then(|labels| labels.first())
        .ok_or_else(|| format!("missing canonical label for {head}"))?;
    let direct_readback = direct_label_readback(head, Path::new(rocksdb_path), label)?;
    Ok(HeadHarvest {
        head,
        fsv_path: path.to_path_buf(),
        sha256: sha256_file(path)?,
        producer_rows: doc["happy_path"]["after_state"]["producer_rows"]
            .as_u64()
            .ok_or_else(|| format!("missing producer_rows for {head}"))?,
        rocksdb_path: Some(rocksdb_path.to_string()),
        cf: doc["source_of_truth"]["cf"].as_str().map(str::to_string),
        direct_readback,
    })
}

fn direct_label_readback(head: &str, rocksdb_path: &Path, label: &Value) -> FsvResult<Value> {
    match head {
        "perf" => {
            let row = required_str(label, "corpus_row_id")?;
            let metric = required_str(label, "metric")?;
            let readback = Q4PerfLabelStore::open(rocksdb_path)?
                .get_label_record(row, metric)?
                .ok_or_else(|| format!("missing direct perf readback for {row}/{metric}"))?;
            require(
                "direct perf readback is a label",
                matches!(&readback.signal, Q4PerfSignalRecord::Label(_)),
            )?;
            Ok(json!({"row": row, "metric": metric, "record": readback}))
        }
        "accuracy" => {
            let row = required_str(label, "corpus_row_id")?;
            let metric = required_str(label, "metric_name")?;
            let readback = Q4AccuracyLabelStore::open(rocksdb_path)?
                .get_label_record(row, metric)?
                .ok_or_else(|| format!("missing direct accuracy readback for {row}/{metric}"))?;
            require(
                "direct accuracy readback is a label",
                matches!(&readback.signal, Q4AccuracySignalRecord::Label(_)),
            )?;
            Ok(json!({"row": row, "metric": metric, "record": readback}))
        }
        "cost" => {
            let row = required_str(label, "corpus_row_id")?;
            let chunk = required_str(label, "chunk_id")?;
            let kind = parse_cost_kind(required_str(label, "kind")?)?;
            let readback = Q4CostLabelStore::open(rocksdb_path)?
                .get_label_record(row, chunk, kind)?
                .ok_or_else(|| format!("missing direct cost readback for {row}/{chunk}"))?;
            require(
                "direct cost readback is a label",
                matches!(&readback.signal, Q4CostSignalRecord::Label(_)),
            )?;
            Ok(json!({"row": row, "chunk": chunk, "kind": kind, "record": readback}))
        }
        other => Err(format!("unsupported direct readback head: {other}").into()),
    }
}

fn required_str<'a>(value: &'a Value, field: &str) -> FsvResult<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| format!("missing string field {field}").into())
}

fn parse_cost_kind(raw: &str) -> FsvResult<Q4CostKind> {
    match raw {
        "ci_minutes" => Ok(Q4CostKind::CiMinutes),
        "dependency_count" => Ok(Q4CostKind::DependencyCount),
        "wheel_bytes" => Ok(Q4CostKind::WheelBytes),
        _ => Err(format!("unknown Q4 cost kind: {raw}").into()),
    }
}

fn edge_missing_perf_fails_closed(catalog: &Q4EvidenceCatalog) -> FsvResult<EdgeCase> {
    let mut catalog = catalog.clone();
    catalog.heads.remove(&Q4HeadKind::Perf);
    let report = evaluate_q4_trust_gate(&catalog)?;
    Ok(EdgeCase {
        name: "missing_perf_fails_closed",
        passed: !report.q4_trust_decision_eligible
            && !report.head(Q4HeadKind::Perf).unwrap().q4_head_ready,
        detail: json!({"q4_trust_gate": report}),
    })
}

fn edge_below_floor_fails_closed(catalog: &Q4EvidenceCatalog) -> FsvResult<EdgeCase> {
    let mut catalog = catalog.clone();
    let perf = catalog
        .heads
        .get_mut(&Q4HeadKind::Perf)
        .ok_or("missing perf evidence")?;
    perf.producer_rows = (Q4_CONFIDENCE_MIN_LABELS - 1) as u64;
    let report = evaluate_q4_trust_gate(&catalog)?;
    Ok(EdgeCase {
        name: "below_floor_producer_fails_closed",
        passed: !report.q4_trust_decision_eligible
            && !report.head(Q4HeadKind::Perf).unwrap().producer_supported,
        detail: json!({"q4_trust_gate": report}),
    })
}

fn edge_wrong_slots_fail_closed(catalog: &Q4EvidenceCatalog) -> FsvResult<EdgeCase> {
    let mut catalog = catalog.clone();
    let cost = catalog
        .heads
        .get_mut(&Q4HeadKind::Cost)
        .ok_or("missing cost evidence")?;
    cost.slots_with_evidence = (0..Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS.len())
        .map(|idx| format!("wrong-slot-{idx}"))
        .collect();
    let report = evaluate_q4_trust_gate(&catalog)?;
    Ok(EdgeCase {
        name: "wrong_slots_fail_closed",
        passed: !report.q4_trust_decision_eligible
            && !report.head(Q4HeadKind::Cost).unwrap().per_slot_supported,
        detail: json!({"q4_trust_gate": report}),
    })
}

fn edge_structural_heads_excluded() -> FsvResult<EdgeCase> {
    let aggregate_heads = aggregate_head_names();
    let expected = [
        "perf",
        "accuracy",
        "cost",
        "reasoning",
        "non_trivial_on_pass",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<BTreeSet<_>>();
    Ok(EdgeCase {
        name: "structural_heads_excluded_until_producers_exist",
        passed: aggregate_heads == expected,
        detail: json!({
            "aggregate_heads": aggregate_heads,
            "excluded_heads": ["security", "edge_case", "latent_bug", "tech_debt", "dead_code", "redundancy"],
        }),
    })
}

fn active_slot_set_is_exact(catalog: &Q4EvidenceCatalog) -> bool {
    let expected = Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    catalog.heads.values().all(|evidence| {
        evidence
            .slots_with_evidence
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected
    })
}

fn aggregate_head_names() -> BTreeSet<String> {
    default_q4_requirements()
        .into_iter()
        .map(|requirement| requirement.head.as_str().to_string())
        .collect()
}

fn write_json_checked<T: Serialize>(path: &Path, value: &T) -> FsvResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    if fs::read(path)? != bytes {
        return Err(format!("{} JSON readback mismatch", path.display()).into());
    }
    Ok(())
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(format!("{:x}", Sha256::digest(&fs::read(path)?)))
}

fn env_path(var: &str, default: &str) -> PathBuf {
    env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn require(name: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("FSV requirement failed: {name}").into())
    }
}
