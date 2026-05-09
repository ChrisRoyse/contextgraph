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
fn split_corpus_keeps_repo_tasks_in_one_bucket_repo_atomic() {
    let entries = corpus_entries();
    let report = split_corpus(
        &entries,
        SplitConfig {
            seed: 7,
            mode: SplitMode::RepoAtomic,
            ..SplitConfig::default()
        },
    )
    .expect("split");

    assert_eq!(report.assignments.len(), entries.len());
    assert!(report.leakage_check_passed);
    assert!(split_is_leakage_free(&report));
    assert_eq!(cross_cut_weights(&report), [0.0, 0.0, 0.0]);
    assert!(report.bucket_sizes.iter().all(|size| *size > 0));
    assert_eq!(report.mode, SplitMode::RepoAtomic);

    let mut repo_to_bucket = BTreeMap::<&str, SplitBucket>::new();
    for assignment in &report.assignments {
        match repo_to_bucket.insert(assignment.repo.as_str(), assignment.bucket) {
            None => {}
            Some(existing) => assert_eq!(existing, assignment.bucket),
        }
    }
}

/// Synthesize SWE-bench Lite's 12-repo, 300-task distribution with the
/// observed [django=114, sympy=77, matplotlib=23, scikit-learn=23,
/// pytest-dev=17, sphinx-doc=16, astropy=6, psf=6, pylint-dev=6,
/// pydata=5, mwaskom=4, pallets=3] sizes and assert InstanceAtomic
/// achieves clean 80/10/10 (within ±1 task tolerance from rounding).
#[test]
fn instance_atomic_achieves_clean_80_10_10_on_swe_bench_lite_shape() {
    let counts: &[(&str, usize)] = &[
        ("django", 114),
        ("sympy", 77),
        ("matplotlib", 23),
        ("scikit-learn", 23),
        ("pytest-dev", 17),
        ("sphinx-doc", 16),
        ("astropy", 6),
        ("psf", 6),
        ("pylint-dev", 6),
        ("pydata", 5),
        ("mwaskom", 4),
        ("pallets", 3),
    ];
    let mut entries = Vec::new();
    for (repo, n) in counts {
        for i in 0..*n {
            entries.push(CorpusEntry {
                task_id: format!("{repo}__sample-{i}"),
                repo: repo.to_string(),
            });
        }
    }
    assert_eq!(entries.len(), 300);

    let report = split_corpus(
        &entries,
        SplitConfig {
            seed: 42,
            mode: SplitMode::InstanceAtomic,
            ..SplitConfig::default()
        },
    )
    .expect("instance-atomic split");

    let [train, cal, hold] = report.bucket_sizes;
    // Targets: 240/30/30. With deterministic bin-pack against the lowest
    // current/target ratio, exactly 240/30/30 is achieved.
    assert_eq!([train, cal, hold], [240, 30, 30]);
    assert_eq!(report.mode, SplitMode::InstanceAtomic);
    assert!(report.leakage_check_passed);
    assert_eq!(report.assignments.len(), 300);
    assert!(report.repo_assignments.is_empty());

    // Every task_id must appear exactly once.
    let mut seen = BTreeMap::new();
    for a in &report.assignments {
        let prior = seen.insert(a.task_id.clone(), a.bucket);
        assert!(prior.is_none(), "duplicate assignment for {}", a.task_id);
    }
}

/// On the same shape, RepoAtomic produces the [195, 77, 28] outcome
/// reported in the 2026-05-09 hardening session. The point of this test
/// is to lock the dataset-shape constraint into the repo so future
/// regressions on the InstanceAtomic default are visible alongside it.
#[test]
fn repo_atomic_locks_in_dataset_shape_constraint_on_swe_bench_lite() {
    let counts: &[(&str, usize)] = &[
        ("django", 114),
        ("sympy", 77),
        ("matplotlib", 23),
        ("scikit-learn", 23),
        ("pytest-dev", 17),
        ("sphinx-doc", 16),
        ("astropy", 6),
        ("psf", 6),
        ("pylint-dev", 6),
        ("pydata", 5),
        ("mwaskom", 4),
        ("pallets", 3),
    ];
    let mut entries = Vec::new();
    for (repo, n) in counts {
        for i in 0..*n {
            entries.push(CorpusEntry {
                task_id: format!("{repo}__sample-{i}"),
                repo: repo.to_string(),
            });
        }
    }

    let report = split_corpus(
        &entries,
        SplitConfig {
            seed: 0,
            mode: SplitMode::RepoAtomic,
            ..SplitConfig::default()
        },
    )
    .expect("repo-atomic split");
    // Every bucket must be non-empty and the leakage check must pass.
    assert!(report.bucket_sizes.iter().all(|s| *s > 0));
    assert!(report.leakage_check_passed);
    let [train, cal, hold] = report.bucket_sizes;
    // Document that the achievable ratio under the dataset shape is
    // structurally bounded — the test does not require exactly 195/77/28
    // because the LPT bin-pack can land on different repo combinations
    // depending on the seed, but the train bucket cannot be ≥240 with
    // whole-repo placement.
    assert!(
        train < 240,
        "RepoAtomic must NOT reach 240/30/30 on Lite shape (got {train})"
    );
    assert_eq!(train + cal + hold, 300);
}

#[test]
fn assert_no_test_patch_leakage_passes_when_unique() {
    let entries = corpus_entries();
    let report = split_corpus(&entries, SplitConfig::default()).expect("split");
    let map: BTreeMap<String, String> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.task_id.clone(), format!("sha-{i}")))
        .collect();
    let res = assert_no_test_patch_leakage(&report.assignments, |task_id| {
        map.get(task_id).map(|s| s.as_str())
    });
    assert!(res.is_ok());
}

#[test]
fn assert_no_test_patch_leakage_rejects_collision() {
    let entries = corpus_entries();
    let report = split_corpus(&entries, SplitConfig::default()).expect("split");
    // Force a collision: assign the same SHA to all tasks.
    let res = assert_no_test_patch_leakage(&report.assignments, |_| Some("dupe-sha"));
    // Expect failure unless every task ended up in the same bucket (which
    // doesn't happen with InstanceAtomic 80/10/10).
    let err = res.unwrap_err();
    assert_eq!(err.code(), "MEJEPA_CORPUS_LEAKAGE_DETECTED");
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
            mode: SplitMode::InstanceAtomic,
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
        mode: SplitMode::RepoAtomic,
    };
    assert_eq!(cross_cut_weights(&report), [1.0, 0.0, 0.0]);
    assert!(!split_is_leakage_free(&report));
}
