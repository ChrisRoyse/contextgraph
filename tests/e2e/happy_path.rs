mod harness;

#[test]
fn task_test_001_happy_path_loop_runs_end_to_end() {
    let root = std::path::PathBuf::from("/mnt/d/contextgraph/fsv/phase-f-e2e-happy-path-fsv");
    let report_path = root.join("happy_path_fsv.json");
    let report = harness::run_e2e_happy_path_fsv(&root).expect("E2E happy path FSV");
    harness::write_report_with_readback(&report, &report_path).expect("write E2E FSV report");
    assert!(report.all_passed);
    assert!(report.readback_equal);
    assert!(report.happy_path.len() >= 7);
    assert!(report.boundary_cases.len() >= 3);
    assert!(report_path.is_file());
}
