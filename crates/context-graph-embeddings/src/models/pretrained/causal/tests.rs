//! Tests for the retired E5 CausalModel production contract.
//!
//! E5 causal remains as a legacy type for ABI compatibility, but the active
//! ME-JEPA registry no longer pins or loads it. These tests must fail closed if
//! a stale causal artifact is accidentally treated as an active pretrained input.

use crate::models::pretrained::shared::pretrained_test_model_path_result;

#[test]
fn test_causal_artifact_is_not_active_pretrained_input() {
    let err = pretrained_test_model_path_result("causal")
        .expect_err("retired E5 causal artifact must be rejected by active test registry");
    assert!(
        err.contains("PRETRAINED_TEST_MODEL_NOT_PINNED"),
        "expected retired E5 to fail at the active registry gate, got: {err}"
    );
    assert!(
        err.contains("causal"),
        "error must name the rejected model_dir for operator diagnosis: {err}"
    );
}

#[test]
fn test_warm_causal_accessor_fails_closed_as_retired() {
    let err = match crate::get_warm_causal_model() {
        Ok(_) => panic!("warm E5 causal accessor must fail closed because E5 is retired"),
        Err(err) => err,
    };
    let err_text = format!("{err:?}");
    assert!(
        err_text.contains("retired") && err_text.contains("disabled"),
        "retired E5 error must be explicit and actionable, got: {err_text}"
    );
}
