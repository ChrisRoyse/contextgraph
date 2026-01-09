//! Tests for NeurotransmitterWeights.

use super::domain::Domain;
use super::neurotransmitter_weights::NeurotransmitterWeights;

#[test]
fn test_nt_new_creates_weights() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    assert_eq!(weights.excitatory, 0.6);
    assert_eq!(weights.inhibitory, 0.3);
    assert_eq!(weights.modulatory, 0.4);
}

#[test]
fn test_nt_new_boundary_values() {
    let min = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
    assert!(min.validate());
    let max = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
    assert!(max.validate());
}

#[test]
fn test_nt_for_domain_code() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Code);
    assert_eq!(weights.excitatory, 0.6);
    assert_eq!(weights.inhibitory, 0.3);
    assert_eq!(weights.modulatory, 0.4);
}

#[test]
fn test_nt_for_domain_legal() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Legal);
    assert_eq!(weights.excitatory, 0.4);
    assert_eq!(weights.inhibitory, 0.4);
    assert_eq!(weights.modulatory, 0.2);
}

#[test]
fn test_nt_for_domain_medical() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Medical);
    assert_eq!(weights.excitatory, 0.5);
    assert_eq!(weights.inhibitory, 0.3);
    assert_eq!(weights.modulatory, 0.5);
}

#[test]
fn test_nt_for_domain_creative() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
    assert_eq!(weights.excitatory, 0.8);
    assert_eq!(weights.inhibitory, 0.1);
    assert_eq!(weights.modulatory, 0.6);
}

#[test]
fn test_nt_for_domain_research() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Research);
    assert_eq!(weights.excitatory, 0.6);
    assert_eq!(weights.inhibitory, 0.2);
    assert_eq!(weights.modulatory, 0.5);
}

#[test]
fn test_nt_for_domain_general() {
    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    assert_eq!(weights.excitatory, 0.5);
    assert_eq!(weights.inhibitory, 0.2);
    assert_eq!(weights.modulatory, 0.3);
}

#[test]
fn test_nt_all_domains_produce_valid_weights() {
    for domain in Domain::all() {
        let weights = NeurotransmitterWeights::for_domain(domain);
        assert!(
            weights.validate(),
            "Domain {:?} produced invalid weights",
            domain
        );
    }
}

#[test]
fn test_nt_compute_effective_weight_general_base_1() {
    let weights = NeurotransmitterWeights::for_domain(Domain::General);
    let effective = weights.compute_effective_weight(1.0);
    assert!(
        (effective - 0.276).abs() < 0.001,
        "Expected ~0.276, got {}",
        effective
    );
}

#[test]
fn test_nt_compute_effective_weight_creative_amplifies() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
    let effective = weights.compute_effective_weight(1.0);
    assert!(
        (effective - 0.728).abs() < 0.001,
        "Expected ~0.728, got {}",
        effective
    );
}

#[test]
fn test_nt_compute_effective_weight_legal_dampens() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Legal);
    let effective = weights.compute_effective_weight(1.0);
    assert!(
        (effective - 0.0).abs() < 0.001,
        "Expected ~0.0, got {}",
        effective
    );
}

#[test]
fn test_nt_compute_effective_weight_clamps_high() {
    let weights = NeurotransmitterWeights::new(1.0, 0.0, 1.0);
    let effective = weights.compute_effective_weight(1.0);
    assert_eq!(effective, 1.0, "Must clamp to 1.0, got {}", effective);
}

#[test]
fn test_nt_compute_effective_weight_clamps_low() {
    let weights = NeurotransmitterWeights::new(0.0, 1.0, 0.0);
    let effective = weights.compute_effective_weight(1.0);
    assert_eq!(effective, 0.0, "Must clamp to 0.0, got {}", effective);
}

#[test]
fn test_nt_compute_effective_weight_zero_base() {
    let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
    let effective = weights.compute_effective_weight(0.0);
    assert_eq!(effective, 0.0, "Zero base should produce zero output");
}

#[test]
fn test_nt_compute_effective_weight_always_in_range() {
    for exc in [0.0, 0.25, 0.5, 0.75, 1.0] {
        for inh in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for modul in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for base in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let weights = NeurotransmitterWeights::new(exc, inh, modul);
                    let effective = weights.compute_effective_weight(base);
                    assert!(
                        (0.0..=1.0).contains(&effective),
                        "Out of range: exc={}, inh={}, mod={}, base={} -> {}",
                        exc,
                        inh,
                        modul,
                        base,
                        effective
                    );
                }
            }
        }
    }
}

#[test]
fn test_nt_validate_valid_weights() {
    let weights = NeurotransmitterWeights::new(0.5, 0.5, 0.5);
    assert!(weights.validate());
}

#[test]
fn test_nt_validate_boundary_valid() {
    let min = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
    let max = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
    assert!(min.validate());
    assert!(max.validate());
}

#[test]
fn test_nt_validate_invalid_excitatory_high() {
    let weights = NeurotransmitterWeights::new(1.1, 0.5, 0.5);
    assert!(!weights.validate());
}

#[test]
fn test_nt_validate_invalid_excitatory_low() {
    let weights = NeurotransmitterWeights::new(-0.1, 0.5, 0.5);
    assert!(!weights.validate());
}

#[test]
fn test_nt_validate_invalid_inhibitory_high() {
    let weights = NeurotransmitterWeights::new(0.5, 1.1, 0.5);
    assert!(!weights.validate());
}

#[test]
fn test_nt_validate_invalid_modulatory_high() {
    let weights = NeurotransmitterWeights::new(0.5, 0.5, 1.1);
    assert!(!weights.validate());
}

#[test]
fn test_nt_validate_nan_excitatory() {
    let weights = NeurotransmitterWeights::new(f32::NAN, 0.5, 0.5);
    assert!(!weights.validate(), "NaN must fail validation per AP-009");
}

#[test]
fn test_nt_validate_nan_inhibitory() {
    let weights = NeurotransmitterWeights::new(0.5, f32::NAN, 0.5);
    assert!(!weights.validate(), "NaN must fail validation per AP-009");
}

#[test]
fn test_nt_validate_nan_modulatory() {
    let weights = NeurotransmitterWeights::new(0.5, 0.5, f32::NAN);
    assert!(!weights.validate(), "NaN must fail validation per AP-009");
}

#[test]
fn test_nt_validate_infinity() {
    let weights = NeurotransmitterWeights::new(f32::INFINITY, 0.5, 0.5);
    assert!(
        !weights.validate(),
        "Infinity must fail validation per AP-009"
    );
}

#[test]
fn test_nt_validate_neg_infinity() {
    let weights = NeurotransmitterWeights::new(f32::NEG_INFINITY, 0.5, 0.5);
    assert!(
        !weights.validate(),
        "Neg infinity must fail validation per AP-009"
    );
}

#[test]
fn test_nt_default_is_general() {
    let default_weights = NeurotransmitterWeights::default();
    let general_weights = NeurotransmitterWeights::for_domain(Domain::General);
    assert_eq!(
        default_weights, general_weights,
        "Default must equal General profile"
    );
}

#[test]
fn test_nt_default_values() {
    let weights = NeurotransmitterWeights::default();
    assert_eq!(weights.excitatory, 0.5);
    assert_eq!(weights.inhibitory, 0.2);
    assert_eq!(weights.modulatory, 0.3);
}

#[test]
fn test_nt_default_is_valid() {
    let weights = NeurotransmitterWeights::default();
    assert!(weights.validate(), "Default weights must be valid");
}

#[test]
fn test_nt_clone() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    let cloned = weights;
    assert_eq!(weights, cloned);
}

#[test]
fn test_nt_copy() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    let copied = weights;
    assert_eq!(weights, copied);
    let _still_valid = weights;
}

#[test]
fn test_nt_debug_format() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    let debug = format!("{:?}", weights);
    assert!(debug.contains("NeurotransmitterWeights"));
    assert!(debug.contains("excitatory"));
}

#[test]
fn test_nt_partial_eq() {
    let w1 = NeurotransmitterWeights::new(0.5, 0.5, 0.5);
    let w2 = NeurotransmitterWeights::new(0.5, 0.5, 0.5);
    let w3 = NeurotransmitterWeights::new(0.6, 0.5, 0.5);
    assert_eq!(w1, w2);
    assert_ne!(w1, w3);
}

#[test]
fn test_nt_serde_roundtrip() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    let json = serde_json::to_string(&weights).expect("serialize failed");
    let restored: NeurotransmitterWeights =
        serde_json::from_str(&json).expect("deserialize failed");
    assert_eq!(weights, restored);
}

#[test]
fn test_nt_serde_json_format() {
    let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
    let json = serde_json::to_string(&weights).unwrap();
    assert!(json.contains("excitatory"));
    assert!(json.contains("inhibitory"));
    assert!(json.contains("modulatory"));
}

#[test]
fn test_nt_serde_all_domain_profiles() {
    for domain in Domain::all() {
        let weights = NeurotransmitterWeights::for_domain(domain);
        let json = serde_json::to_string(&weights).expect("serialize failed");
        let restored: NeurotransmitterWeights =
            serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(weights, restored, "Roundtrip failed for {:?}", domain);
    }
}
