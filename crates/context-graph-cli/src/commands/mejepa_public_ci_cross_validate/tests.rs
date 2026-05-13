use super::*;

#[test]
fn pearson_returns_positive_correlation_for_separated_scores() {
    let samples = vec![(0.90, 1.0), (0.20, 0.0), (0.80, 1.0), (0.10, 0.0)];
    let value = pearson_correlation(&samples).expect("correlation");
    assert!(value > 0.95, "{value}");
}

#[test]
fn pearson_returns_none_for_constant_actuals() {
    let samples = vec![(0.90, 1.0), (0.80, 1.0), (0.70, 1.0)];
    assert_eq!(pearson_correlation(&samples), None);
}

#[test]
fn validate_public_entry_rejects_invalid_prediction_score() {
    let entry = PublicCorpusEntry {
        bucket: "holdout".to_string(),
        category: "known_good".to_string(),
        language: "rust".to_string(),
        mutation_note: None,
        oracle_all_passed: true,
        oracle_exception: None,
        oracle_per_test_count: Some(1),
        oracle_verdict_sha256: None,
        patch_path: "patches/one.diff".to_string(),
        patch_sha256: "0".repeat(64),
        predicted_oracle_pass: 1.5,
        repo: "github.com/example/repo".to_string(),
        task_id: "gha-run-1".to_string(),
    };
    let err = validate_public_entry(&entry, 0).unwrap_err().to_string();
    assert!(err.contains("predicted_oracle_pass"));
}
