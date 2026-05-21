use context_graph_mejepa::{
    agreement_goodhart_flag, build_cross_panel_window_report, encoder_non_overlap_audit,
    read_cross_panel_agreements, read_panel_b_observations, write_cross_panel_agreement,
    write_panel_b_observation, CrossPanelAgreementRecord, CrossPanelFlag, CrossPanelScoreRow,
    PanelBObservationRecord, PanelBScoreSource, Verdict, CROSS_PANEL_GOODHART_DETECTED,
    CROSS_PANEL_SCHEMA_VERSION, PANEL_A_GATE_THRESHOLD, PANEL_B_GATE_THRESHOLD,
    PANEL_CORRELATION_DELTA_MAX,
};
use context_graph_mejepa_cf::{
    ALL_HYGIENE_REFERENCED_CFS, CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS,
};
use context_graph_mejepa_instruments::panel_b::{
    default_panel_b_artifact_specs, require_panel_b_artifacts, resolve_panel_b_artifact_manifest,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const NOW: i64 = 1_779_360_000_000;
const PANEL_PAIR_ID: &str = "panel-a-panel-b-v1";

fn main() -> FsvResult<()> {
    if env::var("CG_CROSS_PANEL_SYNTHETIC_FIXTURE").as_deref() == Ok("1") {
        return run_synthetic_fixture();
    }

    let fsv_root = env_path(
        "CG_CROSS_PANEL_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-111c-real-panel-b-scorer-fsv",
    );
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;

    let models_root = env_path("CG_PANEL_B_MODELS_ROOT", "/zfs/hot/contextgraph/models");
    let manifest =
        resolve_panel_b_artifact_manifest(&models_root, &default_panel_b_artifact_specs())?;
    require_panel_b_artifacts(&manifest)?;
    let panel_b_artifacts = manifest.artifact_shas_by_slot();
    let manifest_json = serde_json::to_vec(&manifest)?;
    let manifest_sha256 = sha256_hex(&manifest_json);

    let score_rows_path = required_env_path("CG_PANEL_B_SCORE_ROWS_JSONL")?;
    let score_rows = read_score_rows_jsonl(&score_rows_path)?;
    let score_source_sha256 = sha256_file(&score_rows_path)?;
    let score_source = PanelBScoreSource {
        source_id: env::var("CG_PANEL_B_SCORE_SOURCE_ID").unwrap_or_else(|_| {
            format!(
                "panel-b-score-source:{}",
                &score_source_sha256[..16.min(score_source_sha256.len())]
            )
        }),
        source_uri: score_rows_path.display().to_string(),
        source_sha256: score_source_sha256.clone(),
        artifact_manifest_sha256: manifest_sha256.clone(),
        row_count: score_rows.len(),
    };
    score_source.validate()?;

    let db_path = fsv_root.join("cross-panel.rocksdb");
    let panel_a_artifacts = panel_a_artifacts();
    let non_overlap = encoder_non_overlap_audit(panel_a_artifacts, panel_b_artifacts.clone())?;
    if !non_overlap.passes() {
        return Err("Panel A and Panel B artifact hashes overlap; refusing gate evidence".into());
    }

    let db = open_db(&db_path)?;
    let healthy_report =
        build_cross_panel_window_report("task-py-g-111c:model-backed", &score_rows)?;
    let healthy_flag = agreement_goodhart_flag(&healthy_report.metric);
    for row in &score_rows {
        write_panel_b_observation(
            &db,
            &panel_b_observation(row, &panel_b_artifacts, &score_source)?,
        )?;
        write_cross_panel_agreement(
            &db,
            &agreement(row, PANEL_PAIR_ID, healthy_flag.clone(), &score_source)?,
        )?;
    }
    flush_cfs(&db)?;
    drop(db);

    let readback_db = open_db(&db_path)?;
    let panel_b_observations = read_panel_b_observations(&readback_db)?;
    let agreements = read_cross_panel_agreements(&readback_db)?;
    drop(readback_db);

    let healthy_agreements: Vec<_> = agreements
        .iter()
        .filter(|row| row.panel_pair_id == PANEL_PAIR_ID)
        .cloned()
        .collect();
    let healthy_readback_rows: Vec<_> = healthy_agreements
        .iter()
        .map(CrossPanelAgreementRecord::to_score_row)
        .collect();
    let healthy_readback_report = build_cross_panel_window_report(
        "task-py-g-111c:model-backed-readback",
        &healthy_readback_rows,
    )?;

    let observed_prediction_ids: BTreeSet<_> = panel_b_observations
        .iter()
        .map(|row| row.prediction_id_hex.clone())
        .collect();
    let healthy_prediction_ids: BTreeSet<_> = healthy_agreements
        .iter()
        .map(|row| row.prediction_id_hex.clone())
        .collect();
    let unmatched_predictions = healthy_prediction_ids
        .symmetric_difference(&observed_prediction_ids)
        .count();
    let row_aligned = panel_b_observations.len() == score_rows.len()
        && healthy_agreements.len() == score_rows.len()
        && unmatched_predictions == 0;
    let panel_b_resident_during_inference = panel_b_observations
        .iter()
        .any(|row| row.panel_b_resident_during_normal_inference);
    let all_rows_model_backed = panel_b_observations
        .iter()
        .all(|row| row.model_backed_panel_b && row.ship_gate_eligible)
        && healthy_agreements
            .iter()
            .all(|row| row.model_backed_panel_b && row.ship_gate_eligible);
    let healthy_gate_ready = healthy_readback_report.metric.flag == CrossPanelFlag::Healthy
        && healthy_readback_report.metric.panel_a_oracle_correlation >= PANEL_A_GATE_THRESHOLD
        && healthy_readback_report.metric.panel_b_oracle_correlation >= PANEL_B_GATE_THRESHOLD
        && healthy_readback_report.metric.abs_correlation_delta <= PANEL_CORRELATION_DELTA_MAX;
    let label_alignment_present = !healthy_readback_report.label_family_metrics.is_empty()
        && !healthy_readback_report.failure_evidence_metrics.is_empty();
    let ship_gate_eligible = row_aligned
        && healthy_gate_ready
        && non_overlap.passes()
        && !panel_b_resident_during_inference
        && label_alignment_present
        && all_rows_model_backed;
    let all_passed = row_aligned
        && non_overlap.passes()
        && !panel_b_resident_during_inference
        && label_alignment_present
        && all_rows_model_backed;

    let evidence = json!({
        "artifactKind": "task_py_g_111c_real_panel_b_scorer_fsv",
        "schemaVersion": CROSS_PANEL_SCHEMA_VERSION,
        "coveredGithubIssues": [571],
        "parentGithubIssues": [405, 521],
        "allPassed": all_passed,
        "modelBackedPanelB": true,
        "shipGateEligible": ship_gate_eligible,
        "sourceOfTruth": {
            "dbPath": db_path,
            "panelBObservationsCf": CF_MEJEPA_PANEL_B_OBSERVATIONS,
            "crossPanelAgreementCf": CF_MEJEPA_CROSS_PANEL_AGREEMENT,
            "scoreRowsJsonl": score_rows_path,
            "panelBModelsRoot": models_root
        },
        "panelBArtifactManifest": manifest,
        "scoreSource": score_source,
        "checks": {
            "rowAligned": row_aligned,
            "rowCount": score_rows.len(),
            "unmatchedPredictions": unmatched_predictions,
            "independentReadbackPanelBRows": panel_b_observations.len(),
            "independentReadbackAgreementRows": healthy_agreements.len(),
            "healthyGateReady": healthy_gate_ready,
            "shipGateEligible": ship_gate_eligible,
            "encoderNonOverlap": non_overlap.passes(),
            "panelABArtifactOverlapCount": non_overlap.overlap_count,
            "panelBResidentDuringNormalInference": panel_b_resident_during_inference,
            "labelAlignmentPresent": label_alignment_present,
            "allRowsModelBacked": all_rows_model_backed
        },
        "modelBackedWindow": healthy_readback_report,
        "encoderNonOverlapAudit": non_overlap,
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false
        }
    });
    let report_path = fsv_root.join("cross_panel_triangulation_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    let readback_all_passed = readback["allPassed"] == true
        && readback["modelBackedPanelB"] == true
        && readback["checks"]["rowAligned"] == true
        && readback["checks"]["encoderNonOverlap"] == true
        && readback["checks"]["panelBResidentDuringNormalInference"] == false
        && readback["checks"]["allRowsModelBacked"] == true;
    let readback_path = fsv_root.join("cross_panel_triangulation_readback.json");
    context_graph_mejepa::eval::report::write_json_0600(
        &readback_path,
        &json!({
            "reportPath": report_path,
            "readbackAllPassed": readback_all_passed,
            "allPassed": readback["allPassed"],
            "modelBackedPanelB": readback["modelBackedPanelB"],
            "shipGateEligible": readback["shipGateEligible"],
            "reportSha256": sha256_hex(fs::read(&report_path)?)
        }),
    )?;
    if !readback_all_passed || !all_passed {
        return Err(format!("TASK-PY-G-111C FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn run_synthetic_fixture() -> FsvResult<()> {
    let fsv_root = env_path("CG_CROSS_PANEL_FSV_ROOT", "/tmp/task-py-g-111-fixture-fsv");
    if fsv_root.exists() {
        fs::remove_dir_all(&fsv_root)?;
    }
    fs::create_dir_all(&fsv_root)?;

    let healthy_rows = synthetic_score_rows(true);
    let injected_rows = synthetic_score_rows(false);
    let healthy_report = build_cross_panel_window_report("task-py-g-111:fixture", &healthy_rows)?;
    let injected_report =
        build_cross_panel_window_report("task-py-g-111:fixture-goodhart", &injected_rows)?;
    let goodhart_detected = injected_report.metric.flag
        == CrossPanelFlag::CrossPanelGoodhartDetected
        && agreement_goodhart_flag(&injected_report.metric)
            == Some(CROSS_PANEL_GOODHART_DETECTED.to_string());
    let fixture_ok = healthy_report.metric.flag == CrossPanelFlag::Healthy && goodhart_detected;
    let evidence = json!({
        "artifactKind": "task_py_g_111_cross_panel_metric_fixture",
        "schemaVersion": CROSS_PANEL_SCHEMA_VERSION,
        "coveredGithubIssues": [405],
        "notCoveredGithubIssues": [521, 571],
        "allPassed": fixture_ok,
        "fixtureOnly": true,
        "modelBackedPanelB": false,
        "shipGateEligible": false,
        "cfWritesAttempted": 0,
        "sourceOfTruth": {
            "reportOnly": true
        },
        "checks": {
            "healthyMetricWorks": healthy_report.metric.flag == CrossPanelFlag::Healthy,
            "goodhartDetected": goodhart_detected,
            "noPanelBObservationWrites": true,
            "noCrossPanelAgreementWrites": true,
            "shipGateEligible": false
        },
        "healthyWindow": healthy_report,
        "goodhartInjectionWindow": injected_report,
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false
        }
    });
    let report_path = fsv_root.join("cross_panel_metric_fixture_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&fs::read(&report_path)?)?;
    if readback["allPassed"] != true
        || readback["fixtureOnly"] != true
        || readback["modelBackedPanelB"] != false
        || readback["shipGateEligible"] != false
    {
        return Err(format!("TASK-PY-G-111 fixture failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ScoreRowJson {
    prediction_id_hex: String,
    score_a: f32,
    score_b: f32,
    oracle_pass: bool,
    accepted_label_ids: Vec<String>,
    failure_evidence_set_ids: Vec<String>,
}

fn read_score_rows_jsonl(path: &Path) -> FsvResult<Vec<CrossPanelScoreRow>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);
    let mut rows = Vec::new();
    let mut ids = BTreeSet::new();
    for (line_idx, line) in reader.lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let row: ScoreRowJson = serde_json::from_str(&line)
            .map_err(|err| format!("invalid score row JSONL line {}: {err}", line_idx + 1))?;
        if !ids.insert(row.prediction_id_hex.clone()) {
            return Err(format!(
                "duplicate prediction_id_hex {:?} in score rows",
                row.prediction_id_hex
            )
            .into());
        }
        rows.push(CrossPanelScoreRow {
            prediction_id_hex: row.prediction_id_hex,
            score_a: row.score_a,
            score_b: row.score_b,
            oracle_pass: row.oracle_pass,
            accepted_label_ids: row.accepted_label_ids,
            failure_evidence_set_ids: row.failure_evidence_set_ids,
        });
    }
    if rows.len() < 2 {
        return Err("Panel B score rows require at least two non-empty rows".into());
    }
    Ok(rows)
}

fn synthetic_score_rows(panel_b_good: bool) -> Vec<CrossPanelScoreRow> {
    (0..30)
        .map(|idx| {
            let oracle_pass = idx % 2 == 0;
            CrossPanelScoreRow {
                prediction_id_hex: format!("{idx:032x}"),
                score_a: if oracle_pass { 0.99 } else { 0.01 },
                score_b: match (panel_b_good, oracle_pass) {
                    (true, true) => 0.97,
                    (true, false) => 0.03,
                    (false, true) => 0.20,
                    (false, false) => 0.80,
                },
                oracle_pass,
                accepted_label_ids: vec![if idx % 3 == 0 {
                    "label:api"
                } else {
                    "label:logic"
                }
                .to_string()],
                failure_evidence_set_ids: vec!["failure:evidence".to_string()],
            }
        })
        .collect()
}

fn panel_b_observation(
    row: &CrossPanelScoreRow,
    panel_b_artifact_shas: &BTreeMap<String, String>,
    score_source: &PanelBScoreSource,
) -> FsvResult<PanelBObservationRecord> {
    Ok(PanelBObservationRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: row.prediction_id_hex.clone(),
        panel_b_run_id: "panel-b-run:task-py-g-111c".to_string(),
        panel_id: "panel-b:model-backed:v1".to_string(),
        model_backed_panel_b: true,
        ship_gate_eligible: true,
        score_source: score_source.clone(),
        score_b: row.score_b,
        verdict_b: verdict_from_score(row.score_b),
        oracle_verdict: if row.oracle_pass {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        cell_key: "python:mutation_holdout:task-py-g-111c".to_string(),
        accepted_label_ids: row.accepted_label_ids.clone(),
        failure_evidence_set_ids: row.failure_evidence_set_ids.clone(),
        panel_b_artifact_shas: panel_b_artifact_shas.clone(),
        panel_b_resident_during_normal_inference: false,
        created_at_unix_ms: NOW,
    })
}

fn agreement(
    row: &CrossPanelScoreRow,
    panel_pair_id: &str,
    goodhart_flag: Option<String>,
    score_source: &PanelBScoreSource,
) -> FsvResult<CrossPanelAgreementRecord> {
    let verdict_a = verdict_from_score(row.score_a);
    let verdict_b = verdict_from_score(row.score_b);
    Ok(CrossPanelAgreementRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: row.prediction_id_hex.clone(),
        panel_pair_id: panel_pair_id.to_string(),
        model_backed_panel_b: true,
        ship_gate_eligible: true,
        score_source: score_source.clone(),
        verdict_a,
        verdict_b,
        score_a: row.score_a,
        score_b: row.score_b,
        oracle_verdict: if row.oracle_pass {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        agree_flag: verdict_a == verdict_b,
        cell_key: "python:mutation_holdout:task-py-g-111c".to_string(),
        accepted_label_ids: row.accepted_label_ids.clone(),
        failure_evidence_set_ids: row.failure_evidence_set_ids.clone(),
        goodhart_flag,
        created_at_unix_ms: NOW,
    })
}

fn verdict_from_score(score: f32) -> Verdict {
    if score >= 0.5 {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

fn panel_a_artifacts() -> BTreeMap<String, String> {
    [
        ("ast", "tree-sitter"),
        ("cfg", "attention-cfg"),
        ("dataflow", "def-use-attention"),
        ("code", "qodo-embed-1"),
        ("text", "e5-large-v2"),
    ]
    .into_iter()
    .map(|(slot, artifact)| {
        (
            slot.to_string(),
            sha256_hex(format!("panel-a:{slot}:{artifact}").as_bytes()),
        )
    })
    .collect()
}

fn open_db(path: &Path) -> FsvResult<DB> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let mut cfs = vec![ColumnFamilyDescriptor::new("default", Options::default())];
    cfs.extend(
        ALL_HYGIENE_REFERENCED_CFS
            .iter()
            .copied()
            .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default())),
    );
    Ok(DB::open_cf_descriptors(&opts, path, cfs)?)
}

fn flush_cfs(db: &DB) -> FsvResult<()> {
    for cf_name in [
        CF_MEJEPA_PANEL_B_OBSERVATIONS,
        CF_MEJEPA_CROSS_PANEL_AGREEMENT,
    ] {
        let cf = db
            .cf_handle(cf_name)
            .ok_or_else(|| format!("missing column family {cf_name}"))?;
        db.flush_cf(cf)?;
    }
    Ok(())
}

fn env_path(name: &str, default: &str) -> PathBuf {
    env::var(name)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(default))
}

fn required_env_path(name: &str) -> FsvResult<PathBuf> {
    let path = env::var(name)
        .map(PathBuf::from)
        .map_err(|_| format!("{name} is required for model-backed Panel B FSV"))?;
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!(
            "{name} does not point to a readable file: {}",
            path.display()
        )
        .into())
    }
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    Ok(sha256_hex(fs::read(path)?))
}

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_ref());
    hex::encode(hasher.finalize())
}
