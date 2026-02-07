//! Integration tests for E5 embed_dual_with_hint()
//!
//! Test Suite 3 from CAUSAL_HINT_INTEGRATION_TESTING.md
//!
//! Note: Full embedding integration is tested through Test Suite 4 (MCP store_memory).
//! These tests verify the bias logic and CausalHint API.

use context_graph_core::traits::{CausalDirectionHint, CausalHint};

/// Test 3.5: Bias Factors API Verification
///
/// Directly verify bias_factors() returns correct values
#[test]
fn test_3_5_bias_factors_api() {
    println!("\n=== Test 3.5: Bias Factors API Verification ===");

    // Test Cause hint
    let cause_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.9, vec![]);
    assert!(cause_hint.is_useful(), "Cause hint should be useful");
    let (cb, eb) = cause_hint.bias_factors();
    assert!((cb - 1.3).abs() < 0.01, "Cause bias should be 1.3, got {}", cb);
    assert!((eb - 0.8).abs() < 0.01, "Effect bias should be 0.8, got {}", eb);
    println!("  Cause hint: cause_bias={}, effect_bias={}", cb, eb);

    // Test Effect hint
    let effect_hint = CausalHint::new(true, CausalDirectionHint::Effect, 0.9, vec![]);
    assert!(effect_hint.is_useful(), "Effect hint should be useful");
    let (cb, eb) = effect_hint.bias_factors();
    assert!((cb - 0.8).abs() < 0.01, "Cause bias should be 0.8, got {}", cb);
    assert!((eb - 1.3).abs() < 0.01, "Effect bias should be 1.3, got {}", eb);
    println!("  Effect hint: cause_bias={}, effect_bias={}", cb, eb);

    // Test Neutral hint
    let neutral_hint = CausalHint::new(true, CausalDirectionHint::Neutral, 0.9, vec![]);
    let (cb, eb) = neutral_hint.bias_factors();
    assert!((cb - 1.0).abs() < 0.01, "Cause bias should be 1.0, got {}", cb);
    assert!((eb - 1.0).abs() < 0.01, "Effect bias should be 1.0, got {}", eb);
    println!("  Neutral hint: cause_bias={}, effect_bias={}", cb, eb);

    // Test non-useful hint (low confidence)
    let low_conf_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.3, vec![]);
    assert!(
        !low_conf_hint.is_useful(),
        "Low confidence hint should not be useful"
    );
    println!("  Low confidence hint is not useful (confidence < 0.5)");

    // Test non-causal hint
    let non_causal_hint = CausalHint::new(false, CausalDirectionHint::Cause, 0.9, vec![]);
    assert!(
        !non_causal_hint.is_useful(),
        "Non-causal hint should not be useful"
    );
    println!("  Non-causal hint is not useful (is_causal = false)");

    println!("[PASS] Test 3.5: All bias factors API verified");
}

/// Test 3.6: Bias Application Simulation
///
/// Verify bias application logic without requiring GPU model
#[test]
fn test_3_6_bias_application_simulation() {
    println!("\n=== Test 3.6: Bias Application Simulation ===");

    // Simulate baseline embedding
    let baseline_cause: Vec<f32> = vec![0.5, 0.3, 0.2, 0.4];
    let baseline_effect: Vec<f32> = vec![0.4, 0.5, 0.3, 0.2];

    // Calculate baseline magnitudes
    let baseline_cause_mag: f32 = baseline_cause.iter().map(|x| x * x).sum::<f32>().sqrt();
    let baseline_effect_mag: f32 = baseline_effect.iter().map(|x| x * x).sum::<f32>().sqrt();

    println!("Baseline magnitudes:");
    println!("  Cause: {}", baseline_cause_mag);
    println!("  Effect: {}", baseline_effect_mag);

    // Simulate Cause hint application (1.3x cause, 0.8x effect)
    let cause_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.9, vec![]);
    let (cause_bias, effect_bias) = cause_hint.bias_factors();

    let biased_cause: Vec<f32> = baseline_cause.iter().map(|x| x * cause_bias).collect();
    let biased_effect: Vec<f32> = baseline_effect.iter().map(|x| x * effect_bias).collect();

    let biased_cause_mag: f32 = biased_cause.iter().map(|x| x * x).sum::<f32>().sqrt();
    let biased_effect_mag: f32 = biased_effect.iter().map(|x| x * x).sum::<f32>().sqrt();

    println!("\nWith Cause hint (1.3x cause, 0.8x effect):");
    println!("  Biased cause magnitude: {}", biased_cause_mag);
    println!("  Biased effect magnitude: {}", biased_effect_mag);

    let cause_ratio = biased_cause_mag / baseline_cause_mag;
    let effect_ratio = biased_effect_mag / baseline_effect_mag;

    println!("  Ratios: cause={}, effect={}", cause_ratio, effect_ratio);

    assert!(
        (cause_ratio - 1.3).abs() < 0.01,
        "Cause should be 1.3x baseline"
    );
    assert!(
        (effect_ratio - 0.8).abs() < 0.01,
        "Effect should be 0.8x baseline"
    );

    // Simulate Effect hint application (0.8x cause, 1.3x effect)
    let effect_hint = CausalHint::new(true, CausalDirectionHint::Effect, 0.9, vec![]);
    let (cause_bias, effect_bias) = effect_hint.bias_factors();

    let biased_cause: Vec<f32> = baseline_cause.iter().map(|x| x * cause_bias).collect();
    let biased_effect: Vec<f32> = baseline_effect.iter().map(|x| x * effect_bias).collect();

    let biased_cause_mag: f32 = biased_cause.iter().map(|x| x * x).sum::<f32>().sqrt();
    let biased_effect_mag: f32 = biased_effect.iter().map(|x| x * x).sum::<f32>().sqrt();

    println!("\nWith Effect hint (0.8x cause, 1.3x effect):");
    println!("  Biased cause magnitude: {}", biased_cause_mag);
    println!("  Biased effect magnitude: {}", biased_effect_mag);

    let cause_ratio = biased_cause_mag / baseline_cause_mag;
    let effect_ratio = biased_effect_mag / baseline_effect_mag;

    assert!(
        (cause_ratio - 0.8).abs() < 0.01,
        "Cause should be 0.8x baseline"
    );
    assert!(
        (effect_ratio - 1.3).abs() < 0.01,
        "Effect should be 1.3x baseline"
    );

    println!("[PASS] Test 3.6: Bias application simulation verified");
}

/// Test 3.7: CausalDirectionHint::from_str
///
/// Verify direction hint parsing
#[test]
fn test_3_7_direction_hint_from_str() {
    println!("\n=== Test 3.7: CausalDirectionHint::from_str ===");

    assert_eq!(
        CausalDirectionHint::from_str("cause"),
        CausalDirectionHint::Cause
    );
    assert_eq!(
        CausalDirectionHint::from_str("Cause"),
        CausalDirectionHint::Cause
    );
    assert_eq!(
        CausalDirectionHint::from_str("CAUSE"),
        CausalDirectionHint::Cause
    );

    assert_eq!(
        CausalDirectionHint::from_str("effect"),
        CausalDirectionHint::Effect
    );
    assert_eq!(
        CausalDirectionHint::from_str("Effect"),
        CausalDirectionHint::Effect
    );
    assert_eq!(
        CausalDirectionHint::from_str("EFFECT"),
        CausalDirectionHint::Effect
    );

    assert_eq!(
        CausalDirectionHint::from_str("neutral"),
        CausalDirectionHint::Neutral
    );
    assert_eq!(
        CausalDirectionHint::from_str("unknown"),
        CausalDirectionHint::Neutral
    );
    assert_eq!(
        CausalDirectionHint::from_str("invalid"),
        CausalDirectionHint::Neutral
    );

    println!("[PASS] Test 3.7: All from_str conversions correct");
}

/// Test 3.8: CausalHint Edge Cases
///
/// Verify edge cases in CausalHint
#[test]
fn test_3_8_causal_hint_edge_cases() {
    println!("\n=== Test 3.8: CausalHint Edge Cases ===");

    // Confidence exactly at threshold (0.5)
    let threshold_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.5, vec![]);
    assert!(
        threshold_hint.is_useful(),
        "Confidence at threshold (0.5) should be useful"
    );
    println!("  Confidence=0.5 is useful: {}", threshold_hint.is_useful());

    // Confidence just below threshold
    let below_hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.49, vec![]);
    assert!(
        !below_hint.is_useful(),
        "Confidence below threshold (0.49) should not be useful"
    );
    println!("  Confidence=0.49 is useful: {}", below_hint.is_useful());

    // Confidence clamping
    let high_conf_hint = CausalHint::new(true, CausalDirectionHint::Cause, 1.5, vec![]);
    assert!(
        high_conf_hint.confidence <= 1.0,
        "Confidence should be clamped to 1.0"
    );
    println!(
        "  Confidence=1.5 clamped to: {}",
        high_conf_hint.confidence
    );

    let negative_conf_hint = CausalHint::new(true, CausalDirectionHint::Cause, -0.5, vec![]);
    assert!(
        negative_conf_hint.confidence >= 0.0,
        "Confidence should be clamped to 0.0"
    );
    println!(
        "  Confidence=-0.5 clamped to: {}",
        negative_conf_hint.confidence
    );

    // Default/neutral hint
    let default_hint = CausalHint::not_causal();
    assert!(!default_hint.is_useful(), "Default hint should not be useful");
    println!("  Default hint is_useful: {}", default_hint.is_useful());

    println!("[PASS] Test 3.8: All edge cases handled correctly");
}

// ============================================================================
// Phase 6 Tests: LLM-Guided Embedding Enhancement
// ============================================================================

/// Test CausalHint.to_guidance() converts correctly.
#[test]
fn test_6_1_causal_hint_to_guidance() {
    println!("\n=== Test 6.1: CausalHint to_guidance() ===");

    // Hint with spans and phrases → should produce guidance
    let hint = CausalHint {
        is_causal: true,
        direction_hint: CausalDirectionHint::Cause,
        confidence: 0.85,
        key_phrases: vec!["cortisol".to_string(), "leads to".to_string()],
        description: Some("Stress causes damage".to_string()),
        cause_spans: vec![(0, 14)],
        effect_spans: vec![(23, 44)],
        asymmetry_strength: 0.9,
    };

    let guidance = hint.to_guidance();
    assert!(guidance.is_some(), "Useful hint with spans should produce guidance");
    let g = guidance.unwrap();
    assert_eq!(g.cause_spans, vec![(0, 14)]);
    assert_eq!(g.effect_spans, vec![(23, 44)]);
    assert_eq!(g.key_phrases.len(), 2);
    assert!((g.asymmetry_strength - 0.9).abs() < f32::EPSILON);
    assert!((g.confidence - 0.85).abs() < f32::EPSILON);
    println!("  Guidance with spans: OK");

    // Non-causal hint → should return None
    let non_causal = CausalHint::not_causal();
    assert!(non_causal.to_guidance().is_none(), "Non-causal hint should produce no guidance");
    println!("  Non-causal hint: None");

    // Low confidence → should return None
    let low_conf = CausalHint {
        is_causal: true,
        direction_hint: CausalDirectionHint::Cause,
        confidence: 0.3,
        key_phrases: vec!["test".to_string()],
        description: None,
        cause_spans: vec![(0, 5)],
        effect_spans: vec![],
        asymmetry_strength: 0.5,
    };
    assert!(low_conf.to_guidance().is_none(), "Low confidence hint should produce no guidance");
    println!("  Low confidence hint: None");

    println!("[PASS] Test 6.1: to_guidance() conversion verified");
}

/// Test EmbeddingHintProvenance struct serialization.
#[test]
fn test_6_2_embedding_hint_provenance_serde() {
    use context_graph_core::traits::EmbeddingHintProvenance;

    println!("\n=== Test 6.2: EmbeddingHintProvenance Serde ===");

    let provenance = EmbeddingHintProvenance {
        hint_applied: true,
        llm_model: Some("Hermes-2-Pro".to_string()),
        static_cause_markers: 3,
        static_effect_markers: 2,
        llm_cause_markers: 1,
        llm_effect_markers: 2,
        direction: Some("cause".to_string()),
        hint_confidence: Some(0.85),
        asymmetry_strength: Some(0.9),
        effective_marker_boost: Some(2.375), // 2.5 * (0.5 + 0.5 * 0.9)
        hint_generated_at: Some(1700000000),
    };

    // Serialize to JSON
    let json = serde_json::to_string(&provenance).unwrap();
    assert!(json.contains("\"hint_applied\":true"));
    assert!(json.contains("\"llm_cause_markers\":1"));
    assert!(json.contains("\"Hermes-2-Pro\""));
    println!("  Serialized: {} bytes", json.len());

    // Roundtrip
    let deser: EmbeddingHintProvenance = serde_json::from_str(&json).unwrap();
    assert_eq!(deser, provenance, "Roundtrip should preserve all fields");
    println!("  Roundtrip: OK");

    // Default
    let default = EmbeddingHintProvenance::default();
    assert!(!default.hint_applied);
    assert_eq!(default.static_cause_markers, 0);
    assert!(default.llm_model.is_none());
    println!("  Default: OK");

    println!("[PASS] Test 6.2: EmbeddingHintProvenance serde verified");
}

/// Test CausalHintGuidance construction and fields.
#[test]
fn test_6_3_causal_hint_guidance_struct() {
    use context_graph_core::traits::CausalHintGuidance;

    println!("\n=== Test 6.3: CausalHintGuidance Struct ===");

    let guidance = CausalHintGuidance {
        cause_spans: vec![(0, 14), (30, 45)],
        effect_spans: vec![(50, 70)],
        key_phrases: vec!["cortisol".to_string(), "hippocampal".to_string()],
        asymmetry_strength: 0.85,
        confidence: 0.9,
    };

    assert_eq!(guidance.cause_spans.len(), 2);
    assert_eq!(guidance.effect_spans.len(), 1);
    assert_eq!(guidance.key_phrases.len(), 2);
    assert!((guidance.asymmetry_strength - 0.85).abs() < f32::EPSILON);

    // Default
    let default = CausalHintGuidance::default();
    assert!(default.cause_spans.is_empty());
    assert!(default.effect_spans.is_empty());
    assert!(default.key_phrases.is_empty());
    assert_eq!(default.asymmetry_strength, 0.0);
    assert_eq!(default.confidence, 0.0);

    println!("[PASS] Test 6.3: CausalHintGuidance struct verified");
}

/// Test new CausalHint fields (cause_spans, effect_spans, asymmetry_strength).
#[test]
fn test_6_4_causal_hint_new_fields() {
    println!("\n=== Test 6.4: CausalHint New Fields ===");

    // Constructor defaults
    let hint = CausalHint::new(true, CausalDirectionHint::Cause, 0.9, vec!["test".to_string()]);
    assert!(hint.cause_spans.is_empty(), "Default cause_spans should be empty");
    assert!(hint.effect_spans.is_empty(), "Default effect_spans should be empty");
    assert_eq!(hint.asymmetry_strength, 0.0, "Default asymmetry_strength should be 0.0");

    // Manual construction with new fields
    let hint_with_spans = CausalHint {
        is_causal: true,
        direction_hint: CausalDirectionHint::Cause,
        confidence: 0.9,
        key_phrases: vec!["cortisol".to_string()],
        description: None,
        cause_spans: vec![(0, 10)],
        effect_spans: vec![(20, 30)],
        asymmetry_strength: 0.8,
    };

    assert_eq!(hint_with_spans.cause_spans.len(), 1);
    assert_eq!(hint_with_spans.effect_spans.len(), 1);
    assert!((hint_with_spans.asymmetry_strength - 0.8).abs() < f32::EPSILON);

    println!("[PASS] Test 6.4: CausalHint new fields verified");
}
