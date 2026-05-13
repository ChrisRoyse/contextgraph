use super::*;

fn run(run_id: &str, passed: bool, hash_byte: char) -> OracleRunObservation {
    OracleRunObservation {
        run_id: run_id.to_string(),
        oracle_all_passed: passed,
        oracle_verdict_sha256: format!("sha256:{}", hash_byte.to_string().repeat(64)),
    }
}

fn task(task_id: &str, runs: Vec<OracleRunObservation>) -> OracleTaskRuns {
    OracleTaskRuns {
        task_id: task_id.to_string(),
        repo: None,
        category: None,
        runs,
    }
}

#[test]
fn deterministic_task_is_not_quarantined() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![task(
            "stable-task",
            vec![
                run("1", true, 'a'),
                run("2", true, 'a'),
                run("3", true, 'a'),
            ],
        )],
    };
    let (reports, config) = build_quarantine_from_audit(&input, 3, 0.05, "test", 1).unwrap();
    assert!(!reports[0].quarantined);
    assert!(config.quarantined_tasks.is_empty());
}

#[test]
fn inconsistent_oracle_task_is_quarantined() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![task(
            "flaky-task",
            vec![
                run("1", true, 'a'),
                run("2", false, 'b'),
                run("3", true, 'a'),
            ],
        )],
    };
    let (reports, config) = build_quarantine_from_audit(&input, 3, 0.05, "test", 1).unwrap();
    assert!(reports[0].quarantined);
    assert_eq!(config.quarantined_tasks[0].task_id, "flaky-task");
}

#[test]
fn missing_third_run_fails_closed() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![task(
            "short-task",
            vec![run("1", true, 'a'), run("2", true, 'a')],
        )],
    };
    let err = build_quarantine_from_audit(&input, 3, 0.05, "test", 1).unwrap_err();
    assert!(err
        .to_string()
        .contains("MEJEPA_ORACLE_FLAKINESS_INSUFFICIENT_RUNS"));
}

#[test]
fn two_run_override_writes_valid_quarantine_config() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![task(
            "two-run-flaky-task",
            vec![run("1", true, 'a'), run("2", false, 'b')],
        )],
    };
    let (reports, config) = build_quarantine_from_audit(&input, 2, 0.05, "test", 1).unwrap();
    assert!(reports[0].quarantined);
    assert_eq!(config.quarantined_tasks[0].oracle_runs, 2);
    config.validate().unwrap();
}
