//! M04-T14a: Validation tests for NT weight validation wrapper.
//!
//! Tests the Result-returning validation wrapper that bridges
//! `NeurotransmitterWeights::validate() -> bool` to `GraphResult<()>`.
//!
//! # Key Requirements
//! - NO MOCK DATA: All tests use real domain profiles
//! - BEFORE/AFTER logging for edge cases
//! - Error contains correct field name and value
//!
//! # Verification Commands
//! ```bash
//! cargo test -p context-graph-graph nt_validation -- --nocapture
//! RUST_LOG=debug cargo test -p context-graph-graph nt_validation -- --nocapture
//! ```

use context_graph_graph::marblestone::{compute_effective_validated, validate_or_error};
use context_graph_graph::{Domain, GraphError, NeurotransmitterWeights};

// ========== Real Domain Validation Tests (NO MOCK DATA) ==========

#[test]
fn test_validate_real_domain_code() {
    // Use REAL domain profile, not mock data
    let weights = NeurotransmitterWeights::for_domain(Domain::Code);
    println!(
        "BEFORE: Validating Code domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(result.is_ok(), "Code domain should validate successfully");
}

#[test]
fn test_validate_real_domain_legal() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Legal);
    println!(
        "BEFORE: Validating Legal domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(result.is_ok(), "Legal domain should validate successfully");
}

#[test]
fn test_validate_real_domain_medical() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Medical);
    println!(
        "BEFORE: Validating Medical domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(
        result.is_ok(),
        "Medical domain should validate successfully"
    );
}

#[test]
fn test_validate_real_domain_creative() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
    println!(
        "BEFORE: Validating Creative domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(
        result.is_ok(),
        "Creative domain should validate successfully"
    );
}

#[test]
fn test_validate_real_domain_research() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Research);
    println!(
        "BEFORE: Validating Research domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(
        result.is_ok(),
        "Research domain should validate successfully"
    );
}

#[test]
fn test_validate_real_domain_general() {
    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    println!(
        "BEFORE: Validating General domain weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    let result = validate_or_error(&weights);
    println!("AFTER: result = {:?}", result);
    assert!(
        result.is_ok(),
        "General domain should validate successfully"
    );
}

#[test]
fn test_validate_all_real_domains() {
    for domain in Domain::all() {
        let weights = NeurotransmitterWeights::for_domain(domain);
        println!("BEFORE: Validating {:?} domain", domain);
        let result = validate_or_error(&weights);
        println!("AFTER: {:?} -> {:?}", domain, result);
        assert!(
            result.is_ok(),
            "Domain {:?} should produce valid weights",
            domain
        );
    }
    println!("All 6 real domains validated successfully ✓");
}

// ========== Error Field Name Tests ==========

#[test]
fn test_invalid_excitatory_returns_field_name() {
    let invalid = NeurotransmitterWeights::new(1.5, 0.5, 0.5);
    println!("BEFORE: Validating invalid excitatory=1.5");
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "excitatory", "Field name should be 'excitatory'");
            assert!(
                (value - 1.5).abs() < 0.001,
                "Value should be 1.5, got {}",
                value
            );
            println!("  Verified: field='excitatory', value=1.5 ✓");
        }
        _ => panic!("Expected InvalidNtWeights error, got {:?}", result),
    }
}

#[test]
fn test_invalid_inhibitory_returns_field_name() {
    let invalid = NeurotransmitterWeights::new(0.5, 2.0, 0.5);
    println!("BEFORE: Validating invalid inhibitory=2.0");
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "inhibitory", "Field name should be 'inhibitory'");
            assert!(
                (value - 2.0).abs() < 0.001,
                "Value should be 2.0, got {}",
                value
            );
            println!("  Verified: field='inhibitory', value=2.0 ✓");
        }
        _ => panic!("Expected InvalidNtWeights error, got {:?}", result),
    }
}

#[test]
fn test_invalid_modulatory_returns_field_name() {
    let invalid = NeurotransmitterWeights::new(0.5, 0.5, -0.5);
    println!("BEFORE: Validating invalid modulatory=-0.5");
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "modulatory", "Field name should be 'modulatory'");
            assert!(
                (value - (-0.5)).abs() < 0.001,
                "Value should be -0.5, got {}",
                value
            );
            println!("  Verified: field='modulatory', value=-0.5 ✓");
        }
        _ => panic!("Expected InvalidNtWeights error, got {:?}", result),
    }
}

#[test]
fn test_error_check_order_excitatory_first() {
    // Multiple invalid fields - excitatory should be caught first
    let invalid = NeurotransmitterWeights::new(1.5, 2.0, -0.5);
    println!("BEFORE: Validating all-invalid weights (1.5, 2.0, -0.5)");
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, .. }) => {
            assert_eq!(
                field, "excitatory",
                "Should catch excitatory first (check order)"
            );
            println!("  Verified: excitatory caught first ✓");
        }
        _ => panic!("Expected InvalidNtWeights error, got {:?}", result),
    }
}

// ========== Edge Case Tests with BEFORE/AFTER Logging ==========

#[test]
fn test_nan_is_invalid() {
    println!("BEFORE: Testing NaN excitatory weight");
    let invalid = NeurotransmitterWeights::new(f32::NAN, 0.5, 0.5);
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "excitatory");
            assert!(value.is_nan(), "Value should be NaN");
            println!("  NaN correctly rejected ✓");
        }
        _ => panic!("NaN should be rejected, got {:?}", result),
    }
}

#[test]
fn test_positive_infinity_is_invalid() {
    println!("BEFORE: Testing +Infinity inhibitory weight");
    let invalid = NeurotransmitterWeights::new(0.5, f32::INFINITY, 0.5);
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "inhibitory");
            assert!(value.is_infinite(), "Value should be infinite");
            println!("  +Infinity correctly rejected ✓");
        }
        _ => panic!("Infinity should be rejected, got {:?}", result),
    }
}

#[test]
fn test_negative_infinity_is_invalid() {
    println!("BEFORE: Testing -Infinity modulatory weight");
    let invalid = NeurotransmitterWeights::new(0.5, 0.5, f32::NEG_INFINITY);
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);

    match result {
        Err(GraphError::InvalidNtWeights { field, value }) => {
            assert_eq!(field, "modulatory");
            assert!(value.is_infinite(), "Value should be infinite");
            println!("  -Infinity correctly rejected ✓");
        }
        _ => panic!("-Infinity should be rejected, got {:?}", result),
    }
}

#[test]
fn test_boundary_values_accepted() {
    println!("BEFORE: Testing boundary min (0.0, 0.0, 0.0)");
    let min = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
    let result = validate_or_error(&min);
    println!("AFTER: min boundary result = {:?}", result);
    assert!(
        result.is_ok(),
        "Boundary min (0.0, 0.0, 0.0) should be valid"
    );

    println!("BEFORE: Testing boundary max (1.0, 1.0, 1.0)");
    let max = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
    let result = validate_or_error(&max);
    println!("AFTER: max boundary result = {:?}", result);
    assert!(
        result.is_ok(),
        "Boundary max (1.0, 1.0, 1.0) should be valid"
    );
}

#[test]
fn test_just_below_zero_is_invalid() {
    // Epsilon below zero
    println!("BEFORE: Testing -0.0001 excitatory (just below zero)");
    let invalid = NeurotransmitterWeights::new(-0.0001, 0.5, 0.5);
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);
    assert!(result.is_err(), "-0.0001 should be invalid");
}

#[test]
fn test_just_above_one_is_invalid() {
    // Epsilon above 1.0
    println!("BEFORE: Testing 1.0001 modulatory (just above one)");
    let invalid = NeurotransmitterWeights::new(0.5, 0.5, 1.0001);
    let result = validate_or_error(&invalid);
    println!("AFTER: result = {:?}", result);
    assert!(result.is_err(), "1.0001 should be invalid");
}

// ========== compute_effective_validated Tests ==========

#[test]
fn test_compute_effective_validated_success() {
    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    println!("BEFORE: compute_effective_validated with General domain, base=1.0");
    let result = compute_effective_validated(&weights, 1.0);
    println!("AFTER: result = {:?}", result);

    let effective = result.expect("Should succeed");
    // General: ((1.0*0.5 - 1.0*0.2) * (1 + (0.3-0.5)*0.4)) = 0.3 * 0.92 = 0.276
    assert!(
        (effective - 0.276).abs() < 0.001,
        "Expected 0.276, got {}",
        effective
    );
    println!("  Verified: 0.276 ± 0.001 ✓");
}

#[test]
fn test_compute_effective_validated_fails_on_invalid() {
    let invalid = NeurotransmitterWeights::new(-0.1, 0.5, 0.5);
    println!("BEFORE: compute_effective_validated with invalid weights");
    let result = compute_effective_validated(&invalid, 1.0);
    println!("AFTER: result = {:?}", result);
    assert!(result.is_err(), "Should fail on invalid weights");
}

#[test]
fn test_compute_effective_validated_code_domain() {
    // Code: e=0.6, i=0.3, m=0.4
    // w_eff = ((1.0*0.6 - 1.0*0.3) * (1 + (0.4-0.5)*0.4)) = 0.3 * 0.96 = 0.288
    let weights = NeurotransmitterWeights::for_domain(Domain::Code);
    println!("BEFORE: compute_effective_validated with Code domain, base=1.0");
    let result = compute_effective_validated(&weights, 1.0);
    println!("AFTER: result = {:?}", result);

    let effective = result.expect("Should succeed");
    assert!(
        (effective - 0.288).abs() < 0.001,
        "Expected 0.288, got {}",
        effective
    );
    println!("  Verified: 0.288 ± 0.001 ✓");
}

#[test]
fn test_compute_effective_validated_with_base_half() {
    // General with base=0.5:
    // w_eff = ((0.5*0.5 - 0.5*0.2) * (1 + (0.3-0.5)*0.4)) = 0.15 * 0.92 = 0.138
    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    println!("BEFORE: compute_effective_validated with General domain, base=0.5");
    let result = compute_effective_validated(&weights, 0.5);
    println!("AFTER: result = {:?}", result);

    let effective = result.expect("Should succeed");
    assert!(
        (effective - 0.138).abs() < 0.001,
        "Expected 0.138, got {}",
        effective
    );
    println!("  Verified: 0.138 ± 0.001 ✓");
}

// ========== Question Mark Operator Tests ==========

#[test]
fn test_question_mark_operator_propagates_error() {
    fn inner_fn() -> context_graph_graph::GraphResult<f32> {
        let weights = NeurotransmitterWeights::new(1.5, 0.0, 0.0);
        validate_or_error(&weights)?; // Should return early with error
        Ok(42.0) // Should not reach here
    }

    let result = inner_fn();
    println!("BEFORE: Testing ? operator error propagation");
    println!("AFTER: result = {:?}", result);
    assert!(
        matches!(result, Err(GraphError::InvalidNtWeights { .. })),
        "Error should propagate via ? operator"
    );
}

#[test]
fn test_question_mark_operator_allows_success() {
    fn inner_fn() -> context_graph_graph::GraphResult<f32> {
        let weights = NeurotransmitterWeights::for_domain(Domain::General);
        validate_or_error(&weights)?; // Should pass
        compute_effective_validated(&weights, 1.0) // Should work
    }

    let result = inner_fn();
    println!("BEFORE: Testing ? operator success path");
    println!("AFTER: result = {:?}", result);
    assert!(result.is_ok(), "Should succeed for valid weights");
    let value = result.unwrap();
    assert!((value - 0.276).abs() < 0.001);
}

// ========== Error Message Formatting Tests ==========

#[test]
fn test_error_display_message_format() {
    let invalid = NeurotransmitterWeights::new(1.5, 0.5, 0.5);
    let result = validate_or_error(&invalid);

    if let Err(err) = result {
        let msg = err.to_string();
        println!("BEFORE: Testing error Display trait");
        println!("AFTER: error message = \"{}\"", msg);

        // Verify message format matches error.rs definition:
        // #[error("Invalid NT weights: {field} = {value} (must be in [0.0, 1.0])")]
        assert!(msg.contains("Invalid NT weights"));
        assert!(msg.contains("excitatory"));
        assert!(msg.contains("1.5"));
        assert!(msg.contains("[0.0, 1.0]"));
        println!("  Error message format verified ✓");
    } else {
        panic!("Expected error");
    }
}
