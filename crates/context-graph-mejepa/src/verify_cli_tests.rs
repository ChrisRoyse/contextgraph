// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::Path;

use super::*;
use crate::types::{FailedGate, VerifyVerdict};

#[test]
fn prediction_passes_returns_none_on_escalate_verdict() {
    // F-016 regression test (#469): EscalateToHuman is NOT a Pass|Fail emission.
    // The verify_cli prediction_passes() must surface None so that
    // run_verify_report skips check_agreement entirely and pushes
    // MEJEPA_VERIFY_VERDICT_NOT_APPROVED rather than fabricating a `false`.
    // Before the F-016 fix, `prediction_pass.unwrap_or(false)` would have
    // passed `false` into check_agreement and tainted agreement_count with a
    // phantom verdict. This regression test guarantees prediction_passes()
    // continues to return None for every EscalateToHuman variant so the
    // upstream `match` arm in run_verify_report skips the agreement check.
    let escalate = VerifyVerdict::EscalateToHuman {
        reality_prediction: None,
        failed_gate: FailedGate::OutOfDistribution {
            ood_score: 0.9,
            threshold: 0.85,
        },
        gates_passed: 0,
    };
    assert!(prediction_passes(&escalate).is_none());
}

#[test]
fn splitmix64_seed_is_deterministic_per_task_id() {
    let first = sample_order("astropy__astropy-12907");
    let second = sample_order("astropy__astropy-12907");
    assert_eq!(first, second);
}

#[test]
fn task_id_validator_rejects_unknown_task() {
    assert!(validate_task_id("missing__repo-999999").is_err());
}

#[test]
fn patch_path_rejects_traversal() {
    assert!(canonicalize_patch_path(Path::new("../../etc/passwd")).is_err());
}

fn sample_order(task: &str) -> Vec<u64> {
    ["a", "b", "c"]
        .into_iter()
        .map(|candidate| splitmix64(seed64(task) ^ seed64(candidate)))
        .collect()
}
