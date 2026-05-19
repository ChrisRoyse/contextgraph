use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{build_no_compensation_trace, NoCompensationTrace};
use context_graph_mejepa_instruments::{InstrumentSlot, PANEL_DIM};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-087-slot-preserving-late-fusion-predictor-fsv";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceScan {
    file: String,
    banned_pattern: String,
    violation_count: usize,
    pass: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCase {
    case: String,
    expected: Value,
    observed: Value,
    pass: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FsvReport {
    task_id: String,
    issue: u32,
    status: String,
    all_passed: bool,
    source_of_truth: Value,
    source_scans: Vec<SourceScan>,
    happy_path: Value,
    edge_cases: Vec<EdgeCase>,
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(
        std::env::var("CG_MEJEPA_TASK_PY_G_087_FSV_ROOT")
            .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let artifacts = fsv_root.join("artifacts");
    fs::create_dir_all(&artifacts).context("create TASK-PY-G-087 artifact dir")?;

    let source_scans = run_source_scans()?;
    let happy_path = run_happy_path(&artifacts)?;
    let edge_cases = vec![
        one_bad_slot_aggregate_pass_case(&artifacts)?,
        non_finite_case()?,
        wrong_dim_case()?,
    ];
    let all_passed = source_scans.iter().all(|scan| scan.pass)
        && happy_path.get("pass").and_then(Value::as_bool) == Some(true)
        && edge_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "one or more TASK-PY-G-087 FSV checks failed");

    let report_path = fsv_root.join("slot_preserving_late_fusion_predictor_fsv.json");
    let report = FsvReport {
        task_id: "TASK-PY-G-087".to_string(),
        issue: 387,
        status: "pass".to_string(),
        all_passed,
        source_of_truth: json!({
            "durable_fsv_root": fsv_root,
            "primary_artifact": report_path,
            "predictor_contract": "per-slot encoders + late fusion + per-slot decoders",
            "auxiliary_head_contract": "per-slot logits + late class fusion",
            "flat_panel_semantics": "packed transport only; source scan rejects early CONCAT/PANEL_DIM semantic projections",
            "expected_panel_dim": PANEL_DIM,
            "expected_slot_count": InstrumentSlot::all().len(),
        }),
        source_scans,
        happy_path,
        edge_cases,
    };
    write_json_0600(&report_path, &report).context("write TASK-PY-G-087 FSV report")?;
    let report_bytes = fs::read(&report_path).context("read TASK-PY-G-087 FSV report")?;
    let readback: FsvReport =
        serde_json::from_slice(&report_bytes).context("decode TASK-PY-G-087 FSV report")?;
    ensure!(
        readback.all_passed && readback.status == "pass",
        "FSV readback did not preserve pass status"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "task": "TASK-PY-G-087",
            "all_passed": true,
            "artifact_path": report_path,
            "artifact_sha256": hex::encode(Sha256::digest(&report_bytes)),
            "slot_count": InstrumentSlot::all().len(),
            "panel_dim": PANEL_DIM
        }))?
    );
    Ok(())
}

fn run_source_scans() -> Result<Vec<SourceScan>> {
    Ok(vec![
        scan_source(
            "crates/context-graph-mejepa/src/predictor/mod.rs",
            "linear(CONCAT_INPUT_DIM",
        )?,
        scan_source(
            "crates/context-graph-mejepa/src/predictor/heads.rs",
            "linear(PANEL_DIM",
        )?,
    ])
}

fn scan_source(path: &str, banned_pattern: &str) -> Result<SourceScan> {
    let contents = fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let violation_count = contents.matches(banned_pattern).count();
    Ok(SourceScan {
        file: path.to_string(),
        banned_pattern: banned_pattern.to_string(),
        violation_count,
        pass: violation_count == 0,
    })
}

fn run_happy_path(artifacts: &Path) -> Result<Value> {
    let target = vec![0.0f32; PANEL_DIM];
    let mut predicted = target.clone();
    for slot in InstrumentSlot::all() {
        let (offset, dim) = slot.extent();
        for value in &mut predicted[offset..offset + dim] {
            *value = 0.001;
        }
    }
    let trace = build_no_compensation_trace(&predicted, &target, 1.0, 0.10)?;
    ensure!(trace.aggregate_passed, "aggregate should pass");
    ensure!(trace.strict_passed, "all slots should pass strict guard");
    ensure!(trace.violated_slots.is_empty(), "happy path has violations");
    let path = artifacts.join("happy_no_compensation_trace.json");
    write_json_0600(&path, &trace)?;
    let readback: NoCompensationTrace = serde_json::from_slice(&fs::read(&path)?)?;
    ensure!(readback == trace, "happy trace readback changed");
    Ok(json!({
        "case": "all_slots_low_residual_pass",
        "pass": true,
        "tracePath": path,
        "strictPassed": trace.strict_passed,
        "slotCount": trace.slots.len(),
        "pairwiseScoresUsed": trace.pairwise_scores_used.len(),
    }))
}

fn one_bad_slot_aggregate_pass_case(artifacts: &Path) -> Result<EdgeCase> {
    let target = vec![0.0f32; PANEL_DIM];
    let mut predicted = target.clone();
    let (offset, dim) = InstrumentSlot::EOracle.extent();
    for value in &mut predicted[offset..offset + dim] {
        *value = 0.10;
    }
    let trace = build_no_compensation_trace(&predicted, &target, 2.0, 0.5)?;
    let path = artifacts.join("one_bad_slot_aggregate_pass_trace.json");
    write_json_0600(&path, &trace)?;
    Ok(EdgeCase {
        case: "one_bad_slot_cannot_be_hidden_by_passing_aggregate".to_string(),
        expected: json!({
            "aggregatePassed": true,
            "strictPassed": false,
            "violatedSlots": ["e_oracle"]
        }),
        observed: json!({
            "tracePath": path,
            "aggregatePassed": trace.aggregate_passed,
            "strictPassed": trace.strict_passed,
            "violatedSlots": trace.violated_slots,
            "pairwiseScoresUsed": trace.pairwise_scores_used.len(),
        }),
        pass: trace.aggregate_passed
            && !trace.strict_passed
            && trace.violated_slots == vec!["e_oracle".to_string()],
    })
}

fn non_finite_case() -> Result<EdgeCase> {
    let target = vec![0.0f32; PANEL_DIM];
    let mut predicted = target.clone();
    predicted[0] = f32::NAN;
    let observed_error = build_no_compensation_trace(&predicted, &target, 1.0, 0.5)
        .err()
        .map(|err| err.code().to_string());
    Ok(EdgeCase {
        case: "non_finite_panel_fails_closed".to_string(),
        expected: json!({"errorCode": "MEJEPA_INFER_NAN_DETECTED"}),
        observed: json!({"errorCode": observed_error}),
        pass: observed_error.as_deref() == Some("MEJEPA_INFER_NAN_DETECTED"),
    })
}

fn wrong_dim_case() -> Result<EdgeCase> {
    let target = vec![0.0f32; PANEL_DIM];
    let predicted = vec![0.0f32; PANEL_DIM - 1];
    let observed_error = build_no_compensation_trace(&predicted, &target, 1.0, 0.5)
        .err()
        .map(|err| err.code().to_string());
    Ok(EdgeCase {
        case: "wrong_dim_panel_fails_closed".to_string(),
        expected: json!({"errorCode": "MEJEPA_INFER_DIM_MISMATCH"}),
        observed: json!({"errorCode": observed_error}),
        pass: observed_error.as_deref() == Some("MEJEPA_INFER_DIM_MISMATCH"),
    })
}
