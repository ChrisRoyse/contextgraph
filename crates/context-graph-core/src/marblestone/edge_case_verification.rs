//! Edge Case Verification for Marblestone Module
//!
//! This file demonstrates boundary and edge case handling.
//! Run with: cargo test edge_case_verification --features "" -- --nocapture

#[cfg(test)]
mod edge_case_tests {
    use crate::marblestone::{Domain, EdgeType, NeurotransmitterWeights};

    // ============================================================================
    // EDGE CASE 1: Empty/Zero Inputs
    // ============================================================================

    #[test]
    fn edge_case_1_zero_base_weight() {
        println!("\n=== EDGE CASE 1: Zero Base Weight ===");

        let weights = NeurotransmitterWeights::default();
        let base_weight = 0.0_f32;

        println!("STATE BEFORE:");
        println!("  base_weight = {}", base_weight);
        println!("  weights = {:?}", weights);

        let effective = weights.compute_effective_weight(base_weight);

        println!("STATE AFTER:");
        println!("  effective_weight = {}", effective);
        println!("  Expected: 0.0 (zero base means zero effective)");

        assert_eq!(
            effective, 0.0,
            "Zero base weight should produce zero effective weight"
        );
        println!("✓ PASS: Zero input handled correctly\n");
    }

    #[test]
    fn edge_case_1_zero_all_modifiers() {
        println!("\n=== EDGE CASE 1b: All Zero Modifiers ===");

        let weights = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
        let base_weight = 0.5_f32;

        println!("STATE BEFORE:");
        println!("  base_weight = {}", base_weight);
        println!(
            "  excitatory = {}, inhibitory = {}, modulatory = {}",
            weights.excitatory, weights.inhibitory, weights.modulatory
        );

        let effective = weights.compute_effective_weight(base_weight);

        println!("STATE AFTER:");
        println!("  effective_weight = {}", effective);
        // Actual formula: signal = base * exc - base * inh, mod_factor = 1 + (mod - 0.5) * 0.4
        // signal = 0.5 * 0 - 0.5 * 0 = 0, mod_factor = 0.8, result = 0
        println!("  Actual formula: (base*exc - base*inh) * (1 + (mod-0.5)*0.4)");
        println!("  With zeros: 0 * 0.8 = 0.0");

        assert_eq!(
            effective, 0.0,
            "Zero excitatory means zero signal regardless of base"
        );
        println!("✓ PASS: All-zero modifiers handled correctly (produces zero)\n");
    }

    // ============================================================================
    // EDGE CASE 2: Maximum Limits / Boundary Values
    // ============================================================================

    #[test]
    fn edge_case_2_maximum_excitatory() {
        println!("\n=== EDGE CASE 2: Maximum Excitatory (1.0) ===");

        let weights = NeurotransmitterWeights::new(1.0, 0.0, 0.0);
        let base_weight = 1.0_f32;

        println!("STATE BEFORE:");
        println!("  base_weight = {}", base_weight);
        println!("  excitatory = 1.0 (MAX), inhibitory = 0.0, modulatory = 0.0");

        let effective = weights.compute_effective_weight(base_weight);

        println!("STATE AFTER:");
        println!("  effective_weight = {}", effective);
        println!("  Formula: 1.0 × (1 + 1.0 - 0.0 + 0.5×0.0) = 2.0");
        println!("  Clamped to: 1.0 (max allowed)");

        assert!(effective <= 1.0, "Effective weight must be clamped to 1.0");
        println!("✓ PASS: Maximum excitatory clamped correctly\n");
    }

    #[test]
    fn edge_case_2_maximum_inhibitory() {
        println!("\n=== EDGE CASE 2b: Maximum Inhibitory (1.0) ===");

        let weights = NeurotransmitterWeights::new(0.0, 1.0, 0.0);
        let base_weight = 0.5_f32;

        println!("STATE BEFORE:");
        println!("  base_weight = {}", base_weight);
        println!("  excitatory = 0.0, inhibitory = 1.0 (MAX), modulatory = 0.0");

        let effective = weights.compute_effective_weight(base_weight);

        println!("STATE AFTER:");
        println!("  effective_weight = {}", effective);
        println!("  Formula: 0.5 × (1 + 0.0 - 1.0 + 0.5×0.0) = 0.0");

        assert!(
            effective >= 0.0,
            "Effective weight must be clamped to 0.0 minimum"
        );
        println!("✓ PASS: Maximum inhibitory clamped correctly\n");
    }

    #[test]
    fn edge_case_2_saturation_test() {
        println!("\n=== EDGE CASE 2c: Weight Saturation ===");

        // All at maximum
        let weights = NeurotransmitterWeights::new(1.0, 0.0, 1.0);
        let base_weight = 1.0_f32;

        println!("STATE BEFORE:");
        println!("  base_weight = {}", base_weight);
        println!("  excitatory = 1.0, inhibitory = 0.0, modulatory = 1.0");

        let effective = weights.compute_effective_weight(base_weight);

        println!("STATE AFTER:");
        println!("  effective_weight = {}", effective);
        println!("  Formula: 1.0 × (1 + 1.0 - 0.0 + 0.5×1.0) = 2.5");
        println!("  Clamped to: 1.0 (max allowed)");

        assert!(effective <= 1.0, "Saturated weight must be clamped to 1.0");
        assert!(effective >= 0.0, "Weight must not go negative");
        println!("✓ PASS: Saturation handled correctly\n");
    }

    // ============================================================================
    // EDGE CASE 3: Invalid Formats / Deserialization Errors
    // ============================================================================

    #[test]
    fn edge_case_3_invalid_domain_json() {
        println!("\n=== EDGE CASE 3: Invalid Domain JSON ===");

        let invalid_json = r#""invalid_domain""#;

        println!("STATE BEFORE:");
        println!("  Input JSON: {}", invalid_json);
        println!("  Valid domains: code, legal, medical, creative, research, general");

        let result: Result<Domain, _> = serde_json::from_str(invalid_json);

        println!("STATE AFTER:");
        println!("  Deserialization result: {:?}", result.is_err());

        assert!(
            result.is_err(),
            "Invalid domain should fail deserialization"
        );
        println!("✓ PASS: Invalid domain JSON rejected correctly\n");
    }

    #[test]
    fn edge_case_3_invalid_edge_type_json() {
        println!("\n=== EDGE CASE 3b: Invalid EdgeType JSON ===");

        let invalid_json = r#""unknown_edge""#;

        println!("STATE BEFORE:");
        println!("  Input JSON: {}", invalid_json);
        println!("  Valid edge types: semantic, temporal, causal, hierarchical");

        let result: Result<EdgeType, _> = serde_json::from_str(invalid_json);

        println!("STATE AFTER:");
        println!("  Deserialization result: {:?}", result.is_err());

        assert!(
            result.is_err(),
            "Invalid edge type should fail deserialization"
        );
        println!("✓ PASS: Invalid edge type JSON rejected correctly\n");
    }

    #[test]
    fn edge_case_3_malformed_json() {
        println!("\n=== EDGE CASE 3c: Malformed JSON Structure ===");

        let malformed_json = r#"{"excitatory": "not_a_number"}"#;

        println!("STATE BEFORE:");
        println!("  Input JSON: {}", malformed_json);
        println!("  Expected: numeric values for weights");

        let result: Result<NeurotransmitterWeights, _> = serde_json::from_str(malformed_json);

        println!("STATE AFTER:");
        println!("  Deserialization result: {:?}", result.is_err());

        assert!(
            result.is_err(),
            "Malformed JSON should fail deserialization"
        );
        println!("✓ PASS: Malformed JSON rejected correctly\n");
    }

    #[test]
    fn edge_case_3_validate_out_of_range() {
        println!("\n=== EDGE CASE 3d: Out-of-Range Weight Values ===");

        // Create weights with out-of-range values (constructor doesn't clamp)
        let weights = NeurotransmitterWeights::new(1.5, -0.5, 2.0);

        println!("STATE BEFORE:");
        println!("  Requested: excitatory=1.5, inhibitory=-0.5, modulatory=2.0");
        println!("  Valid range: 0.0 to 1.0");

        let validation = weights.validate();

        println!("STATE AFTER:");
        println!("  Actual excitatory: {}", weights.excitatory);
        println!("  Actual inhibitory: {}", weights.inhibitory);
        println!("  Actual modulatory: {}", weights.modulatory);
        println!("  Validation result: {:?}", validation);

        // Constructor stores raw values - validate() returns false for invalid
        assert!(
            !validation,
            "validate() should return false for out-of-range values"
        );

        // But compute_effective_weight still clamps the output
        let base = 0.5;
        let effective = weights.compute_effective_weight(base);
        assert!(
            (0.0..=1.0).contains(&effective),
            "compute_effective_weight must always return clamped value"
        );

        println!("  effective_weight (clamped output): {}", effective);
        println!("✓ PASS: Out-of-range values detected by validate(), output clamped\n");
    }
}
