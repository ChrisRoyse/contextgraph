use context_graph_mejepa::{
    agreement_goodhart_flag, build_cross_panel_window_report, encoder_non_overlap_audit,
    read_cross_panel_agreements, read_panel_b_observations, write_cross_panel_agreement,
    write_panel_b_observation, CrossPanelAgreementRecord, CrossPanelFlag, CrossPanelScoreRow,
    PanelBObservationRecord, Verdict, CROSS_PANEL_GOODHART_DETECTED, CROSS_PANEL_SCHEMA_VERSION,
    PANEL_A_GATE_THRESHOLD, PANEL_B_GATE_THRESHOLD, PANEL_CORRELATION_DELTA_MAX,
};
use context_graph_mejepa_cf::{
    ALL_HYGIENE_REFERENCED_CFS, CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const NOW: i64 = 1_779_360_000_000;
const PANEL_PAIR_ID: &str = "panel-a-panel-b-v1";
const INJECTION_PAIR_ID: &str = "panel-a-panel-b-goodhart-injection";

fn main() -> FsvResult<()> {
    let fsv_root = env_path(
        "CG_CROSS_PANEL_FSV_ROOT",
        "/zfs/archive/contextgraph/fsv/task-py-g-111-cross-panel-triangulation-fsv",
    );
    if fsv_root.exists() {
        std::fs::remove_dir_all(&fsv_root)?;
    }
    std::fs::create_dir_all(&fsv_root)?;
    let db_path = fsv_root.join("cross-panel.rocksdb");
    let panel_a_artifacts = panel_a_artifacts();
    let panel_b_artifacts = panel_b_artifacts();
    let non_overlap = encoder_non_overlap_audit(panel_a_artifacts, panel_b_artifacts.clone())?;

    let db = open_db(&db_path)?;
    let healthy_rows = score_rows(true);
    let healthy_report = build_cross_panel_window_report("task-py-g-111:healthy", &healthy_rows)?;
    let healthy_flag = agreement_goodhart_flag(&healthy_report.metric);
    for row in &healthy_rows {
        write_panel_b_observation(&db, &panel_b_observation(row, &panel_b_artifacts)?)?;
        write_cross_panel_agreement(&db, &agreement(row, PANEL_PAIR_ID, healthy_flag.clone())?)?;
    }

    let injected_rows = score_rows(false);
    let injected_report =
        build_cross_panel_window_report("task-py-g-111:goodhart-injection", &injected_rows)?;
    let injected_flag = agreement_goodhart_flag(&injected_report.metric);
    for row in &injected_rows {
        write_cross_panel_agreement(
            &db,
            &agreement(row, INJECTION_PAIR_ID, injected_flag.clone())?,
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
    let injected_agreements: Vec<_> = agreements
        .iter()
        .filter(|row| row.panel_pair_id == INJECTION_PAIR_ID)
        .cloned()
        .collect();
    let healthy_readback_rows: Vec<_> = healthy_agreements
        .iter()
        .map(CrossPanelAgreementRecord::to_score_row)
        .collect();
    let injected_readback_rows: Vec<_> = injected_agreements
        .iter()
        .map(CrossPanelAgreementRecord::to_score_row)
        .collect();
    let healthy_readback_report =
        build_cross_panel_window_report("task-py-g-111:healthy-readback", &healthy_readback_rows)?;
    let injected_readback_report = build_cross_panel_window_report(
        "task-py-g-111:goodhart-injection-readback",
        &injected_readback_rows,
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
    let panel_b_resident_during_inference = panel_b_observations
        .iter()
        .any(|row| row.panel_b_resident_during_normal_inference);
    let goodhart_flag_rows = injected_agreements
        .iter()
        .filter(|row| row.goodhart_flag.as_deref() == Some(CROSS_PANEL_GOODHART_DETECTED))
        .count();

    let same_30_instance_window = panel_b_observations.len() == 30
        && healthy_agreements.len() == 30
        && unmatched_predictions == 0;
    let healthy_gate_ready = healthy_readback_report.metric.flag == CrossPanelFlag::Healthy
        && healthy_readback_report.metric.panel_a_oracle_correlation >= PANEL_A_GATE_THRESHOLD
        && healthy_readback_report.metric.panel_b_oracle_correlation >= PANEL_B_GATE_THRESHOLD
        && healthy_readback_report.metric.abs_correlation_delta <= PANEL_CORRELATION_DELTA_MAX;
    let goodhart_detected = injected_readback_report.metric.flag
        == CrossPanelFlag::CrossPanelGoodhartDetected
        && goodhart_flag_rows == 30;
    let label_alignment_present = healthy_readback_report
        .label_family_metrics
        .keys()
        .any(|key| key == "label:api")
        && healthy_readback_report
            .failure_evidence_metrics
            .keys()
            .any(|key| key == "failure:evidence");
    let all_passed = same_30_instance_window
        && healthy_gate_ready
        && goodhart_detected
        && non_overlap.passes()
        && !panel_b_resident_during_inference
        && label_alignment_present;

    let evidence = json!({
        "artifactKind": "task_py_g_111_cross_panel_triangulation_fsv",
        "schemaVersion": CROSS_PANEL_SCHEMA_VERSION,
        "coveredGithubIssues": [405],
        "allPassed": all_passed,
        "sourceOfTruth": {
            "dbPath": db_path,
            "panelBObservationsCf": CF_MEJEPA_PANEL_B_OBSERVATIONS,
            "crossPanelAgreementCf": CF_MEJEPA_CROSS_PANEL_AGREEMENT
        },
        "checks": {
            "same30InstanceWindow": same_30_instance_window,
            "unmatchedPredictions": unmatched_predictions,
            "independentReadbackPanelBRows": panel_b_observations.len(),
            "independentReadbackHealthyAgreementRows": healthy_agreements.len(),
            "independentReadbackInjectedAgreementRows": injected_agreements.len(),
            "healthyGateReady": healthy_gate_ready,
            "goodhartDetected": goodhart_detected,
            "goodhartFlagRows": goodhart_flag_rows,
            "encoderNonOverlap": non_overlap.passes(),
            "panelABArtifactOverlapCount": non_overlap.overlap_count,
            "panelBResidentDuringNormalInference": panel_b_resident_during_inference,
            "labelAlignmentPresent": label_alignment_present
        },
        "healthyWindow": healthy_readback_report,
        "goodhartInjectionWindow": injected_readback_report,
        "encoderNonOverlapAudit": non_overlap,
        "activePathPolicy": {
            "mntDActiveReadsOrWrites": false
        }
    });
    let report_path = fsv_root.join("cross_panel_triangulation_fsv.json");
    context_graph_mejepa::eval::report::write_json_0600(&report_path, &evidence)?;
    let readback: serde_json::Value = serde_json::from_slice(&std::fs::read(&report_path)?)?;
    let readback_all_passed = readback["allPassed"] == true
        && readback["checks"]["same30InstanceWindow"] == true
        && readback["checks"]["goodhartDetected"] == true
        && readback["checks"]["encoderNonOverlap"] == true
        && readback["checks"]["panelBResidentDuringNormalInference"] == false;
    let readback_path = fsv_root.join("cross_panel_triangulation_readback.json");
    context_graph_mejepa::eval::report::write_json_0600(
        &readback_path,
        &json!({
            "reportPath": report_path,
            "readbackAllPassed": readback_all_passed,
            "allPassed": readback["allPassed"],
            "reportSha256": sha256_hex(&std::fs::read(&report_path)?)
        }),
    )?;
    if !readback_all_passed || !all_passed {
        return Err(format!("TASK-PY-G-111 FSV failed at {}", report_path.display()).into());
    }
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn score_rows(panel_b_good: bool) -> Vec<CrossPanelScoreRow> {
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
) -> FsvResult<PanelBObservationRecord> {
    Ok(PanelBObservationRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: row.prediction_id_hex.clone(),
        panel_b_run_id: "panel-b-run:task-py-g-111".to_string(),
        panel_id: "panel-b:v1".to_string(),
        score_b: row.score_b,
        verdict_b: verdict_from_score(row.score_b),
        oracle_verdict: if row.oracle_pass {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        cell_key: "python:mutation_holdout:task-py-g-111".to_string(),
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
) -> FsvResult<CrossPanelAgreementRecord> {
    let verdict_a = verdict_from_score(row.score_a);
    let verdict_b = verdict_from_score(row.score_b);
    Ok(CrossPanelAgreementRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: row.prediction_id_hex.clone(),
        panel_pair_id: panel_pair_id.to_string(),
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
        cell_key: "python:mutation_holdout:task-py-g-111".to_string(),
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

fn panel_b_artifacts() -> BTreeMap<String, String> {
    [
        ("ast", "codebert-base"),
        ("cfg", "pythia-program-graph"),
        ("dataflow", "joern-data-flow"),
        ("code", "voyage-code-2"),
        ("text", "bge-large-en-v1.5"),
    ]
    .into_iter()
    .map(|(slot, artifact)| {
        (
            slot.to_string(),
            sha256_hex(format!("panel-b:{slot}:{artifact}").as_bytes()),
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

fn sha256_hex(bytes: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes.as_ref());
    hex::encode(hasher.finalize())
}
