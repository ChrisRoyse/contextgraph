#[path = "../../../tests/e2e/harness.rs"]
mod harness;

#[test]
fn phase_f_e2e_happy_path_runs_full_loop() {
    let temp = tempfile::tempdir().expect("tempdir");
    let report = harness::run_e2e_happy_path_fsv(temp.path()).expect("run E2E happy path");
    assert!(report.all_passed);
    assert!(report.readback_equal);
    assert_eq!(report.task_id, "TASK-TEST-001");
    assert!(report
        .happy_path
        .iter()
        .any(|step| step.step == "predict_readback"));
    assert!(report
        .happy_path
        .iter()
        .any(|step| step.step == "feedback_persistence"));
    assert!(report
        .happy_path
        .iter()
        .any(|step| step.step == "observe_heal_pipeline"));
    assert!(report
        .happy_path
        .iter()
        .any(|step| step.step == "improved_prediction_readback"));
    assert!(report.boundary_cases.iter().all(|case| case.pass));
}
