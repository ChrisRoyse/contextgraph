//! M04-T14: Integration tests for NeurotransmitterWeights and Domain re-exports.
//!
//! Verifies that context-graph-graph properly re-exports and integrates
//! NeurotransmitterWeights from context-graph-core.
//!
//! # Constitution Reference
//! - edge_model.nt_weights: Defines weight structure and domain profiles
//! - AP-009: All weights must be in [0.0, 1.0]
//!
//! # Verification Commands
//! ```bash
//! cargo test -p context-graph-graph nt_integration -- --nocapture
//! ```

use context_graph_graph::{Domain, NeurotransmitterWeights};

// ========== Re-export Verification Tests ==========

#[test]
fn test_nt_weights_reexport_from_graph_crate() {
    // Verify NeurotransmitterWeights is accessible from context_graph_graph
    let weights = NeurotransmitterWeights::for_domain(Domain::Code);
    println!(
        "BEFORE: Testing Code domain re-export: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );
    assert_eq!(weights.excitatory, 0.6);
    assert_eq!(weights.inhibitory, 0.3);
    assert_eq!(weights.modulatory, 0.4);
    println!("AFTER: Code domain values verified correctly");
}

#[test]
fn test_domain_reexport_from_graph_crate() {
    // Verify Domain enum is accessible from context_graph_graph
    let all_domains = Domain::all();
    println!("BEFORE: Testing Domain::all() re-export");
    assert_eq!(all_domains.len(), 6);
    assert_eq!(all_domains[0], Domain::Code);
    assert_eq!(all_domains[5], Domain::General);
    println!("AFTER: Domain::all() returns 6 variants as expected");
}

// ========== Domain Profile Verification Tests ==========

#[test]
fn test_all_domain_profiles_correct() {
    // Verify all domain profiles match constitution specification
    // Domain-Specific Profiles (from M04-T14):
    // | Domain | excitatory | inhibitory | modulatory |
    // |--------|------------|------------|------------|
    // | Code | 0.6 | 0.3 | 0.4 |
    // | Legal | 0.4 | 0.4 | 0.2 |
    // | Medical | 0.5 | 0.3 | 0.5 |
    // | Creative | 0.8 | 0.1 | 0.6 |
    // | Research | 0.6 | 0.2 | 0.5 |
    // | General | 0.5 | 0.2 | 0.3 |

    let expected: [(Domain, f32, f32, f32); 6] = [
        (Domain::Code, 0.6, 0.3, 0.4),
        (Domain::Legal, 0.4, 0.4, 0.2),
        (Domain::Medical, 0.5, 0.3, 0.5),
        (Domain::Creative, 0.8, 0.1, 0.6),
        (Domain::Research, 0.6, 0.2, 0.5),
        (Domain::General, 0.5, 0.2, 0.3),
    ];

    for (domain, exp_e, exp_i, exp_m) in expected {
        println!("BEFORE: Testing {:?} domain profile", domain);
        let weights = NeurotransmitterWeights::for_domain(domain);
        println!(
            "  actual: e={}, i={}, m={}",
            weights.excitatory, weights.inhibitory, weights.modulatory
        );
        println!("  expected: e={}, i={}, m={}", exp_e, exp_i, exp_m);

        assert!(
            (weights.excitatory - exp_e).abs() < 0.001,
            "{:?} excitatory mismatch: {} != {}",
            domain,
            weights.excitatory,
            exp_e
        );
        assert!(
            (weights.inhibitory - exp_i).abs() < 0.001,
            "{:?} inhibitory mismatch: {} != {}",
            domain,
            weights.inhibitory,
            exp_i
        );
        assert!(
            (weights.modulatory - exp_m).abs() < 0.001,
            "{:?} modulatory mismatch: {} != {}",
            domain,
            weights.modulatory,
            exp_m
        );
        println!("AFTER: {:?} domain profile verified ✓", domain);
    }
}

// ========== Effective Weight Computation Tests ==========

#[test]
fn test_compute_effective_weight_general_domain() {
    // General: e=0.5, i=0.2, m=0.3
    // w_eff = ((1.0*0.5 - 1.0*0.2) * (1 + (0.3-0.5)*0.4)).clamp(0,1)
    // w_eff = ((0.5 - 0.2) * (1 + (-0.2)*0.4))
    // w_eff = (0.3 * (1 - 0.08))
    // w_eff = (0.3 * 0.92) = 0.276

    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    println!("BEFORE: Computing effective weight for General domain with base=1.0");
    println!(
        "  weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );

    let effective = weights.compute_effective_weight(1.0);
    println!("AFTER: effective = {}", effective);

    let expected = 0.276;
    assert!(
        (effective - expected).abs() < 0.001,
        "General domain effective weight: {} != {} (expected)",
        effective,
        expected
    );
    println!("  Verified: 0.276 ± 0.001 ✓");
}

#[test]
fn test_compute_effective_weight_code_domain() {
    // Code: e=0.6, i=0.3, m=0.4
    // w_eff = ((1.0*0.6 - 1.0*0.3) * (1 + (0.4-0.5)*0.4)).clamp(0,1)
    // w_eff = ((0.6 - 0.3) * (1 + (-0.1)*0.4))
    // w_eff = (0.3 * (1 - 0.04))
    // w_eff = (0.3 * 0.96) = 0.288

    let weights = NeurotransmitterWeights::for_domain(Domain::Code);
    println!("BEFORE: Computing effective weight for Code domain with base=1.0");
    println!(
        "  weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );

    let effective = weights.compute_effective_weight(1.0);
    println!("AFTER: effective = {}", effective);

    let expected = 0.288;
    assert!(
        (effective - expected).abs() < 0.001,
        "Code domain effective weight: {} != {} (expected)",
        effective,
        expected
    );
    println!("  Verified: 0.288 ± 0.001 ✓");
}

#[test]
fn test_compute_effective_weight_creative_domain() {
    // Creative: e=0.8, i=0.1, m=0.6
    // w_eff = ((1.0*0.8 - 1.0*0.1) * (1 + (0.6-0.5)*0.4)).clamp(0,1)
    // w_eff = ((0.8 - 0.1) * (1 + 0.1*0.4))
    // w_eff = (0.7 * (1 + 0.04))
    // w_eff = (0.7 * 1.04) = 0.728

    let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
    println!("BEFORE: Computing effective weight for Creative domain with base=1.0");
    println!(
        "  weights: e={}, i={}, m={}",
        weights.excitatory, weights.inhibitory, weights.modulatory
    );

    let effective = weights.compute_effective_weight(1.0);
    println!("AFTER: effective = {}", effective);

    let expected = 0.728;
    assert!(
        (effective - expected).abs() < 0.001,
        "Creative domain effective weight: {} != {} (expected)",
        effective,
        expected
    );
    println!("  Verified: 0.728 ± 0.001 ✓");
}

#[test]
fn test_compute_effective_weight_with_base_half() {
    // General with base=0.5:
    // w_eff = ((0.5*0.5 - 0.5*0.2) * (1 + (0.3-0.5)*0.4)).clamp(0,1)
    // w_eff = ((0.25 - 0.1) * 0.92)
    // w_eff = (0.15 * 0.92) = 0.138

    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    println!("BEFORE: Computing effective weight for General domain with base=0.5");

    let effective = weights.compute_effective_weight(0.5);
    println!("AFTER: effective = {}", effective);

    let expected = 0.138;
    assert!(
        (effective - expected).abs() < 0.001,
        "General domain base=0.5 effective weight: {} != {} (expected)",
        effective,
        expected
    );
    println!("  Verified: 0.138 ± 0.001 ✓");
}

// ========== Validation Tests ==========

#[test]
fn test_validate_returns_bool_for_valid_weights() {
    println!("BEFORE: Testing validate() returns true for valid weights");
    let valid = NeurotransmitterWeights::new(0.5, 0.3, 0.4);
    let result = valid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(result, "Valid weights should return true");
}

#[test]
fn test_validate_returns_bool_for_invalid_weights() {
    println!("BEFORE: Testing validate() returns false for excitatory=1.5");
    let invalid = NeurotransmitterWeights::new(1.5, 0.0, 0.0);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "Invalid weights (1.5) should return false");
}

#[test]
fn test_all_domains_produce_valid_weights() {
    println!("BEFORE: Testing all domains produce valid weights");
    for domain in Domain::all() {
        let weights = NeurotransmitterWeights::for_domain(domain);
        println!(
            "  Testing {:?}: e={}, i={}, m={}",
            domain, weights.excitatory, weights.inhibitory, weights.modulatory
        );
        assert!(
            weights.validate(),
            "Domain {:?} produces invalid weights: e={}, i={}, m={}",
            domain,
            weights.excitatory,
            weights.inhibitory,
            weights.modulatory
        );
    }
    println!("AFTER: All 6 domains produce valid weights ✓");
}

// ========== Edge Case Tests (BEFORE/AFTER logging) ==========

#[test]
fn test_boundary_values_valid() {
    // Edge Case 1: Boundary values at exactly 0.0 and 1.0
    println!("BEFORE: Testing boundary value weights (0.0, 0.0, 0.0)");
    let min = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
    let min_valid = min.validate();
    println!("AFTER: validate() = {}", min_valid);
    assert!(min_valid, "Boundary min (0.0, 0.0, 0.0) should be valid");

    println!("BEFORE: Testing boundary value weights (1.0, 1.0, 1.0)");
    let max = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
    let max_valid = max.validate();
    println!("AFTER: validate() = {}", max_valid);
    assert!(max_valid, "Boundary max (1.0, 1.0, 1.0) should be valid");
}

#[test]
fn test_negative_weight_invalid() {
    // Edge Case 2: Out of Range (Negative)
    println!("BEFORE: Testing negative excitatory weight -0.1");
    let invalid = NeurotransmitterWeights::new(-0.1, 0.5, 0.5);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "Negative excitatory (-0.1) should be invalid");
}

#[test]
fn test_above_one_weight_invalid() {
    // Edge Case 3: Out of Range (Above 1.0)
    println!("BEFORE: Testing excitatory weight 1.1 (above max)");
    let invalid = NeurotransmitterWeights::new(1.1, 0.5, 0.5);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "Excitatory above 1.0 (1.1) should be invalid");
}

#[test]
fn test_nan_weight_invalid() {
    // Edge Case: NaN value
    println!("BEFORE: Testing NaN excitatory weight");
    let invalid = NeurotransmitterWeights::new(f32::NAN, 0.5, 0.5);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "NaN excitatory should be invalid");
}

#[test]
fn test_infinity_weight_invalid() {
    // Edge Case: Infinity value
    println!("BEFORE: Testing +Infinity inhibitory weight");
    let invalid = NeurotransmitterWeights::new(0.5, f32::INFINITY, 0.5);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "+Infinity inhibitory should be invalid");
}

#[test]
fn test_negative_infinity_weight_invalid() {
    // Edge Case: Negative Infinity value
    println!("BEFORE: Testing -Infinity modulatory weight");
    let invalid = NeurotransmitterWeights::new(0.5, 0.5, f32::NEG_INFINITY);
    let result = invalid.validate();
    println!("AFTER: validate() = {}", result);
    assert!(!result, "-Infinity modulatory should be invalid");
}

// ========== Default and Constructor Tests ==========

#[test]
fn test_default_is_general_domain() {
    println!("BEFORE: Testing Default trait returns General domain profile");
    let default = NeurotransmitterWeights::default();
    let general = NeurotransmitterWeights::for_domain(Domain::General);

    assert_eq!(default.excitatory, general.excitatory);
    assert_eq!(default.inhibitory, general.inhibitory);
    assert_eq!(default.modulatory, general.modulatory);
    println!("AFTER: Default == General domain ✓");
}

#[test]
fn test_new_constructor_preserves_values() {
    println!("BEFORE: Testing new() constructor preserves input values");
    let weights = NeurotransmitterWeights::new(0.7, 0.2, 0.5);
    assert_eq!(weights.excitatory, 0.7);
    assert_eq!(weights.inhibitory, 0.2);
    assert_eq!(weights.modulatory, 0.5);
    println!("AFTER: Constructor preserves (0.7, 0.2, 0.5) ✓");
}

// ========== Domain Enumeration Tests ==========

#[test]
fn test_domain_description_not_empty() {
    println!("BEFORE: Testing all domains have non-empty descriptions");
    for domain in Domain::all() {
        let desc = domain.description();
        println!("  {:?}: \"{}\"", domain, desc);
        assert!(
            !desc.is_empty(),
            "Domain {:?} has empty description",
            domain
        );
    }
    println!("AFTER: All domains have descriptions ✓");
}

#[test]
fn test_domain_display_lowercase() {
    println!("BEFORE: Testing Domain::fmt() produces lowercase strings");
    let domain = Domain::Code;
    let display = domain.to_string();
    println!("  Code.to_string() = \"{}\"", display);
    assert_eq!(display, "code");

    let general = Domain::General;
    let general_display = general.to_string();
    println!("  General.to_string() = \"{}\"", general_display);
    assert_eq!(general_display, "general");
    println!("AFTER: Domain display is lowercase ✓");
}

// ========== Clamping Behavior Tests ==========

#[test]
fn test_effective_weight_clamped_to_zero() {
    // Create weights where effective would go negative without clamping
    // High inhibitory, low excitatory: (0.1 - 0.9) * 1.0 = -0.8 -> clamped to 0.0
    println!("BEFORE: Testing effective weight clamps to 0.0 for high inhibitory");
    let weights = NeurotransmitterWeights::new(0.1, 0.9, 0.5);
    let effective = weights.compute_effective_weight(1.0);
    println!("AFTER: effective = {} (expected 0.0)", effective);
    assert_eq!(effective, 0.0, "Negative effective should clamp to 0.0");
}

#[test]
fn test_effective_weight_clamped_to_one() {
    // Create weights where effective might exceed 1.0
    // Very high excitatory with high modulatory boost
    println!("BEFORE: Testing effective weight clamps to 1.0 for extreme values");
    let weights = NeurotransmitterWeights::new(1.0, 0.0, 1.0);
    // (1.0*1.0 - 1.0*0.0) * (1 + (1.0-0.5)*0.4) = 1.0 * 1.2 = 1.2 -> clamped to 1.0
    let effective = weights.compute_effective_weight(1.0);
    println!("AFTER: effective = {} (expected 1.0)", effective);
    assert_eq!(effective, 1.0, "Effective > 1.0 should clamp to 1.0");
}
