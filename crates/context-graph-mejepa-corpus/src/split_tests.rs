use super::*;
use std::collections::BTreeMap;

fn corpus_entries() -> Vec<CorpusEntry> {
    let mut entries = Vec::new();
    for repo_idx in 0..16 {
        entries.push(CorpusEntry {
            task_id: format!("single_repo_{repo_idx}__task"),
            repo: format!("single_repo_{repo_idx}"),
        });
    }
    for task_idx in 0..2 {
        entries.push(CorpusEntry {
            task_id: format!("shared_alpha__task_{task_idx}"),
            repo: "shared_alpha".to_string(),
        });
        entries.push(CorpusEntry {
            task_id: format!("shared_beta__task_{task_idx}"),
            repo: "shared_beta".to_string(),
        });
    }
    entries
}

#[test]
fn split_corpus_keeps_repo_tasks_in_one_bucket() {
    let entries = corpus_entries();
    let report = split_corpus(
        &entries,
        SplitConfig {
            seed: 7,
            ..SplitConfig::default()
        },
    )
    .expect("split");

    assert_eq!(report.assignments.len(), entries.len());
    assert!(report.leakage_check_passed);
    assert!(split_is_leakage_free(&report));
    assert_eq!(cross_cut_weights(&report), [0.0, 0.0, 0.0]);
    assert!(report.bucket_sizes.iter().all(|size| *size > 0));

    let mut repo_to_bucket = BTreeMap::<&str, SplitBucket>::new();
    for assignment in &report.assignments {
        match repo_to_bucket.insert(assignment.repo.as_str(), assignment.bucket) {
            None => {}
            Some(existing) => assert_eq!(existing, assignment.bucket),
        }
    }
}

#[test]
fn split_corpus_is_deterministic_for_seed() {
    let entries = corpus_entries();
    let config = SplitConfig {
        seed: 99,
        ..SplitConfig::default()
    };
    let first = split_corpus(&entries, config).expect("first");
    let second = split_corpus(&entries, config).expect("second");
    assert_eq!(first.bucket_sizes, second.bucket_sizes);
    assert_eq!(first.assignments, second.assignments);
    assert_eq!(first.repo_assignments, second.repo_assignments);
}

#[test]
fn split_corpus_rejects_invalid_inputs() {
    let empty = split_corpus(&[], SplitConfig::default()).unwrap_err();
    assert_eq!(empty.code(), "MEJEPA_CORPUS_INVALID_INPUT");

    let entries = corpus_entries();
    let bad_sum = split_corpus(
        &entries,
        SplitConfig {
            train_fraction: 0.7,
            calibration_fraction: 0.2,
            holdout_fraction: 0.2,
            seed: 0,
        },
    )
    .unwrap_err();
    assert_eq!(bad_sum.code(), "MEJEPA_CORPUS_INVALID_INPUT");

    let nan = split_corpus(
        &entries,
        SplitConfig {
            train_fraction: f64::NAN,
            ..SplitConfig::default()
        },
    )
    .unwrap_err();
    assert_eq!(nan.code(), "MEJEPA_CORPUS_INVALID_INPUT");

    let duplicate_task_id = split_corpus(
        &[
            CorpusEntry {
                task_id: "dup".to_string(),
                repo: "repo-a".to_string(),
            },
            CorpusEntry {
                task_id: "dup".to_string(),
                repo: "repo-b".to_string(),
            },
            CorpusEntry {
                task_id: "unique".to_string(),
                repo: "repo-c".to_string(),
            },
        ],
        SplitConfig::default(),
    )
    .unwrap_err();
    assert_eq!(duplicate_task_id.code(), "MEJEPA_CORPUS_INVALID_INPUT");

    let whitespace_repo = split_corpus(
        &[
            CorpusEntry {
                task_id: "task-a".to_string(),
                repo: " repo-a".to_string(),
            },
            CorpusEntry {
                task_id: "task-b".to_string(),
                repo: "repo-b".to_string(),
            },
            CorpusEntry {
                task_id: "task-c".to_string(),
                repo: "repo-c".to_string(),
            },
        ],
        SplitConfig::default(),
    )
    .unwrap_err();
    assert_eq!(whitespace_repo.code(), "MEJEPA_CORPUS_INVALID_INPUT");
}

#[test]
fn cross_cut_weights_detect_manual_repo_leakage() {
    let report = SplitReport {
        assignments: vec![
            TaskAssignment {
                task_id: "a1".to_string(),
                repo: "same_repo".to_string(),
                bucket: SplitBucket::Train,
            },
            TaskAssignment {
                task_id: "a2".to_string(),
                repo: "same_repo".to_string(),
                bucket: SplitBucket::Calibration,
            },
            TaskAssignment {
                task_id: "b1".to_string(),
                repo: "other_repo".to_string(),
                bucket: SplitBucket::Holdout,
            },
        ],
        bucket_sizes: [1, 1, 1],
        bucket_fractions: [1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0],
        repo_assignments: Vec::new(),
        leakage_check_passed: false,
    };
    assert_eq!(cross_cut_weights(&report), [1.0, 0.0, 0.0]);
    assert!(!split_is_leakage_free(&report));
}
