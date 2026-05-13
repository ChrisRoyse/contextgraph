#[path = "../../../tests/e2e/load.rs"]
mod load;

#[test]
fn task_test_007_load_harness_persists_prediction_and_feedback_metrics() {
    let fsv_root = load::default_fsv_root();
    let report_path = load::default_report_path(&fsv_root);
    let report = load::run_load_fsv(&fsv_root).expect("load FSV");
    load::write_report_with_readback(&report, &report_path).expect("write load report");
    let expected_rows = (report.profile.concurrent_agents * report.profile.edits_per_agent) as u64;

    assert!(report.all_passed);
    assert!(report.readback_equal);
    assert!(report.happy_path.len() >= 4);
    assert!(report.boundary_cases.len() >= 3);
    assert_eq!(
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
            .copied(),
        Some(expected_rows)
    );
    assert_eq!(
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
            .copied(),
        Some(expected_rows)
    );
}
