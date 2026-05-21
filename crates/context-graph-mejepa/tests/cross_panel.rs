use context_graph_mejepa::{
    agreement_goodhart_flag, build_cross_panel_window_report, cross_panel_metric,
    encoder_non_overlap_audit, write_cross_panel_agreement, write_panel_b_observation,
    CrossPanelAgreementRecord, CrossPanelFlag, CrossPanelScoreRow, PanelBObservationRecord,
    PanelBScoreSource, Verdict, CROSS_PANEL_GOODHART_DETECTED, CROSS_PANEL_SCHEMA_VERSION,
};
use context_graph_mejepa_cf::{
    ALL_HYGIENE_REFERENCED_CFS, CF_MEJEPA_CROSS_PANEL_AGREEMENT, CF_MEJEPA_PANEL_B_OBSERVATIONS,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::collections::BTreeMap;
use tempfile::TempDir;

fn rows(panel_b_good: bool) -> Vec<CrossPanelScoreRow> {
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
                    "label:api".to_string()
                } else {
                    "label:logic".to_string()
                }],
                failure_evidence_set_ids: vec!["failure:evidence".to_string()],
            }
        })
        .collect()
}

#[test]
fn cross_panel_metric_marks_healthy_when_panels_agree() {
    let metric = cross_panel_metric(&rows(true)).unwrap();
    assert_eq!(metric.flag, CrossPanelFlag::Healthy);
    assert!(metric.panel_a_oracle_correlation >= 0.99);
    assert!(metric.panel_b_oracle_correlation >= 0.99);
}

#[test]
fn cross_panel_metric_detects_panel_a_only_goodhart() {
    let metric = cross_panel_metric(&rows(false)).unwrap();
    assert_eq!(metric.flag, CrossPanelFlag::CrossPanelGoodhartDetected);
    assert_eq!(
        agreement_goodhart_flag(&metric),
        Some(CROSS_PANEL_GOODHART_DETECTED.to_string())
    );
}

#[test]
fn window_report_groups_by_label_and_failure_evidence() {
    let report = build_cross_panel_window_report("window:test", &rows(true)).unwrap();
    assert_eq!(report.metric.n, 30);
    assert!(report.label_family_metrics.contains_key("label:api"));
    assert!(report.label_family_metrics.contains_key("label:logic"));
    assert!(report
        .failure_evidence_metrics
        .contains_key("failure:evidence"));
}

#[test]
fn encoder_non_overlap_audit_counts_shared_sha() {
    let mut panel_a = BTreeMap::new();
    panel_a.insert("ast".to_string(), "a".repeat(64));
    let mut panel_b = BTreeMap::new();
    panel_b.insert("ast".to_string(), "b".repeat(64));
    let clean = encoder_non_overlap_audit(panel_a.clone(), panel_b).unwrap();
    assert!(clean.passes());

    let mut panel_b_overlap = BTreeMap::new();
    panel_b_overlap.insert("ast".to_string(), "a".repeat(64));
    let overlap = encoder_non_overlap_audit(panel_a, panel_b_overlap).unwrap();
    assert_eq!(overlap.overlap_count, 1);
    assert!(!overlap.passes());
}

#[test]
fn panel_b_observation_rejects_fixture_score_source() {
    let temp = TempDir::new().unwrap();
    let db = test_db(temp.path());
    let mut record = panel_b_observation("fixture-score-source");
    record.score_source.source_id = "synthetic-hardcoded-fixture".to_string();
    let err = write_panel_b_observation(&db, &record).unwrap_err();
    assert!(err.to_string().contains("synthetic"));
    assert!(read_cf_count(&db, CF_MEJEPA_PANEL_B_OBSERVATIONS) == 0);
}

#[test]
fn cross_panel_agreement_rejects_non_model_backed_rows_before_cf_write() {
    let temp = TempDir::new().unwrap();
    let db = test_db(temp.path());
    let mut record = cross_panel_agreement("not-model-backed");
    record.model_backed_panel_b = false;
    let err = write_cross_panel_agreement(&db, &record).unwrap_err();
    assert!(err.to_string().contains("model-backed"));
    assert!(read_cf_count(&db, CF_MEJEPA_CROSS_PANEL_AGREEMENT) == 0);
}

#[test]
fn model_backed_records_write_and_read_back() {
    let temp = TempDir::new().unwrap();
    let db = test_db(temp.path());
    let observation = panel_b_observation("real-model-backed-source");
    let agreement = cross_panel_agreement("real-model-backed-source");
    write_panel_b_observation(&db, &observation).unwrap();
    write_cross_panel_agreement(&db, &agreement).unwrap();
    assert_eq!(read_cf_count(&db, CF_MEJEPA_PANEL_B_OBSERVATIONS), 1);
    assert_eq!(read_cf_count(&db, CF_MEJEPA_CROSS_PANEL_AGREEMENT), 1);
}

fn panel_b_observation(source_id: &str) -> PanelBObservationRecord {
    PanelBObservationRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: "a".repeat(32),
        panel_b_run_id: "panel-b-run:test".to_string(),
        panel_id: "panel-b:model-backed:test".to_string(),
        model_backed_panel_b: true,
        ship_gate_eligible: true,
        score_source: score_source(source_id),
        score_b: 0.97,
        verdict_b: Verdict::Pass,
        oracle_verdict: Verdict::Pass,
        cell_key: "python:test".to_string(),
        accepted_label_ids: vec!["label:api".to_string()],
        failure_evidence_set_ids: vec!["failure:evidence".to_string()],
        panel_b_artifact_shas: artifact_shas(),
        panel_b_resident_during_normal_inference: false,
        created_at_unix_ms: 1,
    }
}

fn cross_panel_agreement(source_id: &str) -> CrossPanelAgreementRecord {
    CrossPanelAgreementRecord {
        schema_version: CROSS_PANEL_SCHEMA_VERSION,
        prediction_id_hex: "b".repeat(32),
        panel_pair_id: "panel-a-panel-b:test".to_string(),
        model_backed_panel_b: true,
        ship_gate_eligible: true,
        score_source: score_source(source_id),
        verdict_a: Verdict::Pass,
        verdict_b: Verdict::Pass,
        score_a: 0.99,
        score_b: 0.97,
        oracle_verdict: Verdict::Pass,
        agree_flag: true,
        cell_key: "python:test".to_string(),
        accepted_label_ids: vec!["label:api".to_string()],
        failure_evidence_set_ids: vec!["failure:evidence".to_string()],
        goodhart_flag: None,
        created_at_unix_ms: 1,
    }
}

fn score_source(source_id: &str) -> PanelBScoreSource {
    PanelBScoreSource {
        source_id: source_id.to_string(),
        source_uri: "/zfs/hot/contextgraph/models/panel-b/score-rows.jsonl".to_string(),
        source_sha256: "c".repeat(64),
        artifact_manifest_sha256: "d".repeat(64),
        row_count: 2,
    }
}

fn artifact_shas() -> BTreeMap<String, String> {
    let mut shas = BTreeMap::new();
    shas.insert("ast".to_string(), "e".repeat(64));
    shas.insert("text".to_string(), "f".repeat(64));
    shas
}

fn test_db(path: &std::path::Path) -> DB {
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
    DB::open_cf_descriptors(&opts, path, cfs).unwrap()
}

fn read_cf_count(db: &DB, cf_name: &str) -> usize {
    let cf = db.cf_handle(cf_name).unwrap();
    db.iterator_cf(cf, rocksdb::IteratorMode::Start).count()
}
