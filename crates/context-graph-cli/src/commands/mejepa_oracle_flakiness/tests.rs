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

fn task_with_category(
    task_id: &str,
    category: &str,
    runs: Vec<OracleRunObservation>,
) -> OracleTaskRuns {
    OracleTaskRuns {
        task_id: task_id.to_string(),
        repo: Some("repo/name".to_string()),
        category: Some(category.to_string()),
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

#[test]
fn same_task_in_distinct_categories_is_audited_per_row() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![
            task_with_category(
                "same-task",
                "over_engineer",
                vec![run("stable-1", true, 'a'), run("stable-2", true, 'a')],
            ),
            task_with_category(
                "same-task",
                "known_good",
                vec![run("flaky-1", false, 'b'), run("flaky-2", true, 'c')],
            ),
        ],
    };

    let (reports, config) = build_quarantine_from_audit(&input, 2, 0.05, "test", 1).unwrap();

    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].audit_key, "same-task|over_engineer");
    assert!(!reports[0].quarantined);
    assert_eq!(reports[1].audit_key, "same-task|known_good");
    assert!(reports[1].quarantined);
    assert_eq!(config.quarantined_tasks.len(), 1);
    assert_eq!(config.quarantined_tasks[0].task_id, "same-task");
    assert!(config.quarantined_tasks[0]
        .reason
        .contains("same-task|known_good"));
}

#[test]
fn duplicate_task_category_rows_fail_closed() {
    let input = OracleFlakinessAuditInput {
        tasks: vec![
            task_with_category(
                "duplicate-task",
                "known_good",
                vec![run("1", true, 'a'), run("2", true, 'a')],
            ),
            task_with_category(
                "duplicate-task",
                "known_good",
                vec![run("3", true, 'a'), run("4", true, 'a')],
            ),
        ],
    };

    let err = build_quarantine_from_audit(&input, 2, 0.05, "test", 1).unwrap_err();
    assert!(err
        .to_string()
        .contains("MEJEPA_ORACLE_FLAKINESS_DUPLICATE_ROW"));
}
