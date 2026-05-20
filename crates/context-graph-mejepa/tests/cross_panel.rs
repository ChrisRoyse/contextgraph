use context_graph_mejepa::{
    agreement_goodhart_flag, build_cross_panel_window_report, cross_panel_metric,
    encoder_non_overlap_audit, CrossPanelFlag, CrossPanelScoreRow, CROSS_PANEL_GOODHART_DETECTED,
};
use std::collections::BTreeMap;

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
