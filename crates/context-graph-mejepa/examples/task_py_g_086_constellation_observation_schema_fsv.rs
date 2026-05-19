use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    expected_panel_dim, expected_slot_count, ConstellationObservation,
    PairwiseRelationshipObservation, PanelId, SlotProvenance,
};
use context_graph_mejepa_instruments::{hash_f32s, InstrumentSlot, Panel, PanelBuilder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-086-constellation-observation-schema-fsv";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EdgeCase {
    case: String,
    expected: Value,
    before_state: Value,
    after_state: Value,
    observed_error_code: Option<String>,
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
    before_state: Value,
    happy_path: Value,
    edge_cases: Vec<EdgeCase>,
    after_state: Value,
}

fn main() -> Result<()> {
    let fsv_root = PathBuf::from(
        std::env::var("CG_MEJEPA_TASK_PY_G_086_FSV_ROOT")
            .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let artifacts = fsv_root.join("artifacts");
    fs::create_dir_all(&artifacts).context("create FSV artifact dir")?;

    let report_path = fsv_root.join("constellation_observation_schema_fsv.json");
    let before_state = source_state(&fsv_root)?;
    let happy_path = run_happy_path(&artifacts).context("happy path")?;
    let edge_cases = vec![
        missing_slot_case(&artifacts)?,
        duplicate_slot_case(&artifacts)?,
        wrong_dim_case(&artifacts)?,
        non_finite_case(&artifacts)?,
        unknown_field_case(&artifacts)?,
    ];
    let all_passed = happy_path.get("pass").and_then(Value::as_bool) == Some(true)
        && edge_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "one or more TASK-PY-G-086 FSV cases failed");

    let report = FsvReport {
        task_id: "TASK-PY-G-086".to_string(),
        issue: 386,
        status: "pass".to_string(),
        all_passed,
        source_of_truth: json!({
            "durable_fsv_root": fsv_root,
            "primary_artifact": report_path,
            "readback_mode": "write observation artifacts, then reopen JSON files and revalidate schema/hash state",
            "schema": "ConstellationObservation",
            "expected_slot_count": expected_slot_count(),
            "expected_panel_dim": expected_panel_dim(),
        }),
        before_state,
        happy_path,
        edge_cases,
        after_state: source_state(&fsv_root)?,
    };
    write_json_0600(&report_path, &report).context("write FSV report")?;
    let report_bytes = fs::read(&report_path).context("read FSV report")?;
    let readback: FsvReport = serde_json::from_slice(&report_bytes).context("decode FSV report")?;
    ensure!(
        readback.all_passed && readback.status == "pass",
        "FSV report readback did not preserve pass status"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "task": "TASK-PY-G-086",
            "all_passed": true,
            "artifact_path": report_path,
            "artifact_sha256": hex::encode(Sha256::digest(&report_bytes)),
            "slot_count": expected_slot_count(),
            "panel_dim": expected_panel_dim()
        }))?
    );
    Ok(())
}

fn run_happy_path(artifacts: &Path) -> Result<Value> {
    let before = source_state(artifacts)?;
    let (panel, provenance) = panel_fixture(None)?;
    let expected_hashes = expected_hashes(&panel);
    let observation = ConstellationObservation::from_panel(
        PanelId("synthetic-panel-complete".to_string()),
        "synthetic-chunk-complete".to_string(),
        "mutation=known_good|language=python".to_string(),
        &panel,
        &provenance,
        vec![
            pair("e_ast", "e_cfg", 0.77),
            pair("e_oracle", "scalars", -0.12),
        ],
    )?;
    ensure!(
        observation.filled_slot_count() == expected_slot_count(),
        "happy observation must fill all slots"
    );
    ensure!(
        observation.slot_hashes() == expected_hashes,
        "slot hash map must match direct panel slice hashing"
    );
    observation.validate()?;

    let observation_path = artifacts.join("happy_constellation_observation.json");
    let hashes_path = artifacts.join("happy_slot_hashes.json");
    write_json_0600(&observation_path, &observation)?;
    write_json_0600(&hashes_path, &expected_hashes)?;

    let readback: ConstellationObservation = serde_json::from_slice(&fs::read(&observation_path)?)?;
    readback.validate()?;
    ensure!(readback == observation, "observation JSON readback changed");
    let hash_readback: BTreeMap<String, String> = serde_json::from_slice(&fs::read(&hashes_path)?)?;
    ensure!(
        hash_readback == expected_hashes,
        "slot hash JSON readback changed"
    );

    Ok(json!({
        "case": "complete_panel_preserves_slot_identity_and_hashes",
        "pass": true,
        "before": before,
        "after": source_state(artifacts)?,
        "observationPath": observation_path,
        "slotHashesPath": hashes_path,
        "readbackFilledSlots": readback.filled_slot_count(),
        "readbackPairwiseRows": readback.pairwise_relationships.len(),
        "firstSlotHash": readback.slot_hashes().get("e_ast"),
    }))
}

fn missing_slot_case(artifacts: &Path) -> Result<EdgeCase> {
    let before = source_state(artifacts)?;
    let (panel, provenance) = panel_fixture(Some(InstrumentSlot::ETrace))?;
    let observation = ConstellationObservation::from_panel(
        PanelId("synthetic-panel-missing-e-trace".to_string()),
        "synthetic-chunk-missing".to_string(),
        "mutation=known_good|language=python".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )?;
    let missing = observation.missing_slot_ids();
    let path = artifacts.join("edge_missing_slot_observation.json");
    write_json_0600(&path, &observation)?;
    let readback: ConstellationObservation = serde_json::from_slice(&fs::read(&path)?)?;
    let pass = readback.validate().is_ok()
        && missing == vec!["e_trace".to_string()]
        && readback.filled_slot_count() == expected_slot_count() - 1;
    Ok(EdgeCase {
        case: "missing_slot_is_enumerated_not_hidden".to_string(),
        expected: json!({"missingSlot": "e_trace", "validObservation": true}),
        before_state: before,
        after_state: source_state(artifacts)?,
        observed_error_code: None,
        pass,
    })
}

fn duplicate_slot_case(artifacts: &Path) -> Result<EdgeCase> {
    let before = source_state(artifacts)?;
    let (panel, provenance) = panel_fixture(None)?;
    let mut observation = ConstellationObservation::from_panel(
        PanelId("synthetic-panel-duplicate".to_string()),
        "synthetic-chunk-duplicate".to_string(),
        "mutation=known_good|language=python".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )?;
    observation.slots[1].slot_id = observation.slots[0].slot_id.clone();
    let observed = observation
        .validate()
        .err()
        .map(|err| err.code().to_string());
    Ok(EdgeCase {
        case: "duplicate_slot_rejects".to_string(),
        expected: json!({"errorCode": "MEJEPA_INFER_INVALID_INPUT", "noArtifactWritten": true}),
        before_state: before,
        after_state: source_state(artifacts)?,
        observed_error_code: observed.clone(),
        pass: observed.as_deref() == Some("MEJEPA_INFER_INVALID_INPUT"),
    })
}

fn wrong_dim_case(artifacts: &Path) -> Result<EdgeCase> {
    let before = source_state(artifacts)?;
    let (panel, provenance) = panel_fixture(None)?;
    let mut observation = ConstellationObservation::from_panel(
        PanelId("synthetic-panel-wrong-dim".to_string()),
        "synthetic-chunk-wrong-dim".to_string(),
        "mutation=known_good|language=python".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )?;
    observation.slots[0].dim += 1;
    let observed = observation
        .validate()
        .err()
        .map(|err| err.code().to_string());
    Ok(EdgeCase {
        case: "wrong_dimension_rejects".to_string(),
        expected: json!({"errorCode": "MEJEPA_INFER_DIM_MISMATCH", "noArtifactWritten": true}),
        before_state: before,
        after_state: source_state(artifacts)?,
        observed_error_code: observed.clone(),
        pass: observed.as_deref() == Some("MEJEPA_INFER_DIM_MISMATCH"),
    })
}

fn non_finite_case(artifacts: &Path) -> Result<EdgeCase> {
    let before = source_state(artifacts)?;
    let mut data = vec![0.0_f32; expected_panel_dim()];
    data[InstrumentSlot::EAst.offset()] = f32::NAN;
    let observed = Panel::try_new(data, 1)
        .err()
        .map(|err| err.code().to_string());
    Ok(EdgeCase {
        case: "non_finite_panel_rejects_before_observation".to_string(),
        expected: json!({"errorCode": "MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT", "noArtifactWritten": true}),
        before_state: before,
        after_state: source_state(artifacts)?,
        observed_error_code: observed.clone(),
        pass: observed.as_deref() == Some("MEJEPA_INSTRUMENTS_NUMERICAL_INVARIANT"),
    })
}

fn unknown_field_case(artifacts: &Path) -> Result<EdgeCase> {
    let before = source_state(artifacts)?;
    let (panel, provenance) = panel_fixture(None)?;
    let mut value = serde_json::to_value(ConstellationObservation::from_panel(
        PanelId("synthetic-panel-unknown-field".to_string()),
        "synthetic-chunk-unknown-field".to_string(),
        "mutation=known_good|language=python".to_string(),
        &panel,
        &provenance,
        Vec::new(),
    )?)?;
    value["unexpected"] = json!(true);
    let err = serde_json::from_value::<ConstellationObservation>(value).unwrap_err();
    let pass = err.to_string().contains("unknown field");
    Ok(EdgeCase {
        case: "unknown_json_field_rejects".to_string(),
        expected: json!({"serdeDenyUnknownFields": true}),
        before_state: before,
        after_state: source_state(artifacts)?,
        observed_error_code: Some("SERDE_UNKNOWN_FIELD".to_string()),
        pass,
    })
}

fn panel_fixture(
    missing: Option<InstrumentSlot>,
) -> Result<(Panel, BTreeMap<InstrumentSlot, SlotProvenance>)> {
    let mut builder = PanelBuilder::new();
    let mut provenance = BTreeMap::new();
    for slot in InstrumentSlot::all() {
        if Some(slot) == missing {
            continue;
        }
        let vector: Vec<f32> = (0..slot.dim())
            .map(|idx| ((slot.offset() + idx + 1) as f32) / 10_000.0)
            .collect();
        builder.set_slot(slot, &vector)?;
        provenance.insert(
            slot,
            SlotProvenance {
                instrument_id: format!("instrument:{}", slot.slug()),
                model_version_hash: format!("synthetic-model-version-{}", slot.slug()),
                source_evidence_id: format!("synthetic-source-evidence-{}", slot.slug()),
                calibration_cell: "mutation=known_good|language=python".to_string(),
            },
        );
    }
    Ok((builder.build()?, provenance))
}

fn expected_hashes(panel: &Panel) -> BTreeMap<String, String> {
    InstrumentSlot::all()
        .into_iter()
        .filter(|slot| panel.is_filled(*slot))
        .map(|slot| (slot.slug().to_string(), hash_f32s(panel.slot(slot))))
        .collect()
}

fn pair(left: &str, right: &str, correlation: f32) -> PairwiseRelationshipObservation {
    PairwiseRelationshipObservation {
        left_slot_id: left.to_string(),
        right_slot_id: right.to_string(),
        relationship_kind: "synthetic_pairwise_correlation".to_string(),
        correlation: Some(correlation),
        mutual_information: Some(correlation.abs() * 0.5),
        source_evidence_id: format!("synthetic-pairwise-{left}-{right}"),
    }
}

fn source_state(path: &Path) -> Result<Value> {
    let entries = if path.exists() {
        fs::read_dir(path)?
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().to_string()))
            .collect::<std::result::Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    Ok(json!({
        "path": path,
        "exists": path.exists(),
        "entryCount": entries.len(),
        "entries": entries,
    }))
}
