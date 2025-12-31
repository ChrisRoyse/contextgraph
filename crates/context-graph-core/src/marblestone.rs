//! Marblestone architecture integration for context-aware neurotransmitter weighting.
//!
//! This module provides domain classification for the Marblestone edge model,
//! enabling context-specific retrieval behavior in the knowledge graph.
//!
//! # Constitution Reference
//! - edge_model.nt_weights.domain: Code|Legal|Medical|Creative|Research|General
//! - Formula: w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)

use serde::{Deserialize, Serialize};
use std::fmt;

/// Knowledge domain for context-aware neurotransmitter weighting.
///
/// Different domains have different optimal retrieval characteristics:
/// - Code: High precision, structured relationships
/// - Legal: High inhibition, careful reasoning
/// - Medical: High causal awareness, evidence-based
/// - Creative: High exploration, associative connections
/// - Research: Balanced exploration and precision
/// - General: Default balanced profile
///
/// # Constitution Compliance
/// - Naming: PascalCase enum per constitution.yaml
/// - Serde: snake_case serialization per JSON naming rules
///
/// # Example
/// ```rust
/// use context_graph_core::marblestone::Domain;
///
/// let domain = Domain::Code;
/// assert_eq!(domain.to_string(), "code");
/// assert!(domain.description().contains("precision"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    /// Programming and software development context.
    /// Characteristics: High precision, structured relationships, strong type awareness.
    Code,
    /// Legal documents and reasoning context.
    /// Characteristics: High inhibition, careful reasoning, precedent-based.
    Legal,
    /// Medical and healthcare context.
    /// Characteristics: High causal awareness, evidence-based, risk-conscious.
    Medical,
    /// Creative writing and artistic context.
    /// Characteristics: High exploration, associative connections, novelty-seeking.
    Creative,
    /// Academic research context.
    /// Characteristics: Balanced exploration and precision, citation-aware.
    Research,
    /// General purpose context.
    /// Characteristics: Default balanced profile for mixed contexts.
    General,
}

impl Domain {
    /// Returns a human-readable description of this domain's characteristics.
    ///
    /// # Returns
    /// Static string describing the domain's retrieval behavior.
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::Domain;
    ///
    /// let desc = Domain::Medical.description();
    /// assert!(desc.contains("causal"));
    /// ```
    #[inline]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Code => "High precision, structured relationships, strong type awareness",
            Self::Legal => "High inhibition, careful reasoning, precedent-based",
            Self::Medical => "High causal awareness, evidence-based, risk-conscious",
            Self::Creative => "High exploration, associative connections, novelty-seeking",
            Self::Research => "Balanced exploration and precision, citation-aware",
            Self::General => "Default balanced profile for mixed contexts",
        }
    }

    /// Returns all domain variants as an array.
    ///
    /// # Returns
    /// Array containing all 6 Domain variants in definition order.
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::Domain;
    ///
    /// let all = Domain::all();
    /// assert_eq!(all.len(), 6);
    /// assert_eq!(all[0], Domain::Code);
    /// assert_eq!(all[5], Domain::General);
    /// ```
    #[inline]
    pub fn all() -> [Domain; 6] {
        [
            Self::Code,
            Self::Legal,
            Self::Medical,
            Self::Creative,
            Self::Research,
            Self::General,
        ]
    }
}

impl Default for Domain {
    /// Returns `Domain::General` as the default.
    ///
    /// General is the most balanced profile, suitable for mixed contexts.
    #[inline]
    fn default() -> Self {
        Self::General
    }
}

impl fmt::Display for Domain {
    /// Formats the domain as a lowercase string.
    ///
    /// # Output
    /// - Code → "code"
    /// - Legal → "legal"
    /// - Medical → "medical"
    /// - Creative → "creative"
    /// - Research → "research"
    /// - General → "general"
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Code => "code",
            Self::Legal => "legal",
            Self::Medical => "medical",
            Self::Creative => "creative",
            Self::Research => "research",
            Self::General => "general",
        };
        write!(f, "{}", s)
    }
}

/// Neurotransmitter-inspired weight modulation for graph edges.
///
/// Based on the Marblestone architecture, edges are modulated by three signals:
/// - **Excitatory**: Strengthens connections (analogous to glutamate)
/// - **Inhibitory**: Weakens connections (analogous to GABA)
/// - **Modulatory**: Context-dependent adjustment (analogous to dopamine/serotonin)
///
/// # Constitution Reference
/// - edge_model.nt_weights section
/// - All weights must be in [0.0, 1.0] per AP-009
///
/// # Example
/// ```rust
/// use context_graph_core::marblestone::{NeurotransmitterWeights, Domain};
///
/// let weights = NeurotransmitterWeights::for_domain(Domain::Code);
/// let effective = weights.compute_effective_weight(0.8);
/// assert!(effective >= 0.0 && effective <= 1.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NeurotransmitterWeights {
    /// Excitatory signal strength [0.0, 1.0]. Higher = stronger connection.
    pub excitatory: f32,
    /// Inhibitory signal strength [0.0, 1.0]. Higher = weaker connection.
    pub inhibitory: f32,
    /// Modulatory signal strength [0.0, 1.0]. Context-dependent adjustment.
    pub modulatory: f32,
}

impl NeurotransmitterWeights {
    /// Create new weights with explicit values.
    ///
    /// # Arguments
    /// * `excitatory` - Strengthening signal [0.0, 1.0]
    /// * `inhibitory` - Weakening signal [0.0, 1.0]
    /// * `modulatory` - Domain-adjustment signal [0.0, 1.0]
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::NeurotransmitterWeights;
    ///
    /// let weights = NeurotransmitterWeights::new(0.7, 0.2, 0.5);
    /// assert_eq!(weights.excitatory, 0.7);
    /// ```
    #[inline]
    pub fn new(excitatory: f32, inhibitory: f32, modulatory: f32) -> Self {
        Self { excitatory, inhibitory, modulatory }
    }

    /// Get domain-specific neurotransmitter profile.
    ///
    /// Each domain has optimized NT weights for its retrieval characteristics:
    /// - **Code**: excitatory=0.6, inhibitory=0.3, modulatory=0.4 (precise)
    /// - **Legal**: excitatory=0.4, inhibitory=0.4, modulatory=0.2 (conservative)
    /// - **Medical**: excitatory=0.5, inhibitory=0.3, modulatory=0.5 (causal)
    /// - **Creative**: excitatory=0.8, inhibitory=0.1, modulatory=0.6 (exploratory)
    /// - **Research**: excitatory=0.6, inhibitory=0.2, modulatory=0.5 (balanced)
    /// - **General**: excitatory=0.5, inhibitory=0.2, modulatory=0.3 (default)
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::{NeurotransmitterWeights, Domain};
    ///
    /// let creative = NeurotransmitterWeights::for_domain(Domain::Creative);
    /// assert_eq!(creative.excitatory, 0.8);
    /// assert_eq!(creative.inhibitory, 0.1);
    /// ```
    #[inline]
    pub fn for_domain(domain: Domain) -> Self {
        match domain {
            Domain::Code => Self::new(0.6, 0.3, 0.4),
            Domain::Legal => Self::new(0.4, 0.4, 0.2),
            Domain::Medical => Self::new(0.5, 0.3, 0.5),
            Domain::Creative => Self::new(0.8, 0.1, 0.6),
            Domain::Research => Self::new(0.6, 0.2, 0.5),
            Domain::General => Self::new(0.5, 0.2, 0.3),
        }
    }

    /// Compute effective weight given a base weight.
    ///
    /// # Formula
    /// ```text
    /// w_eff = ((base * excitatory - base * inhibitory) * (1 + (modulatory - 0.5) * 0.4)).clamp(0.0, 1.0)
    /// ```
    ///
    /// This applies:
    /// 1. Excitatory amplification: `base * excitatory`
    /// 2. Inhibitory dampening: `base * inhibitory`
    /// 3. Modulatory context adjustment: centered at 0.5, ±20% range
    /// 4. Final clamp to [0.0, 1.0] per AP-009
    ///
    /// # Arguments
    /// * `base_weight` - Original edge weight [0.0, 1.0]
    ///
    /// # Returns
    /// Effective weight always in [0.0, 1.0]
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::{NeurotransmitterWeights, Domain};
    ///
    /// let weights = NeurotransmitterWeights::for_domain(Domain::General);
    /// let effective = weights.compute_effective_weight(1.0);
    /// // General: (1.0*0.5 - 1.0*0.2) * (1 + (0.3-0.5)*0.4) = 0.3 * 0.92 = 0.276
    /// assert!((effective - 0.276).abs() < 0.001);
    /// ```
    #[inline]
    pub fn compute_effective_weight(&self, base_weight: f32) -> f32 {
        // Step 1: Apply excitatory and inhibitory
        let signal = base_weight * self.excitatory - base_weight * self.inhibitory;
        // Step 2: Apply modulatory adjustment (centered at 0.5)
        let mod_factor = 1.0 + (self.modulatory - 0.5) * 0.4;
        // Step 3: Clamp to valid range per AP-009
        (signal * mod_factor).clamp(0.0, 1.0)
    }

    /// Validate that all weights are in valid range [0.0, 1.0].
    ///
    /// # Returns
    /// `true` if all weights are in [0.0, 1.0] and not NaN/Infinity
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::NeurotransmitterWeights;
    ///
    /// let valid = NeurotransmitterWeights::new(0.5, 0.3, 0.4);
    /// assert!(valid.validate());
    ///
    /// let invalid = NeurotransmitterWeights::new(1.5, 0.0, 0.0);
    /// assert!(!invalid.validate());
    /// ```
    #[inline]
    pub fn validate(&self) -> bool {
        // Check for NaN/Infinity per AP-009
        if self.excitatory.is_nan() || self.excitatory.is_infinite() {
            return false;
        }
        if self.inhibitory.is_nan() || self.inhibitory.is_infinite() {
            return false;
        }
        if self.modulatory.is_nan() || self.modulatory.is_infinite() {
            return false;
        }
        // Check valid range [0.0, 1.0]
        self.excitatory >= 0.0 && self.excitatory <= 1.0
            && self.inhibitory >= 0.0 && self.inhibitory <= 1.0
            && self.modulatory >= 0.0 && self.modulatory <= 1.0
    }
}

impl Default for NeurotransmitterWeights {
    /// Returns General domain profile: excitatory=0.5, inhibitory=0.2, modulatory=0.3
    ///
    /// # Example
    /// ```rust
    /// use context_graph_core::marblestone::NeurotransmitterWeights;
    ///
    /// let weights = NeurotransmitterWeights::default();
    /// assert_eq!(weights.excitatory, 0.5);
    /// assert_eq!(weights.inhibitory, 0.2);
    /// assert_eq!(weights.modulatory, 0.3);
    /// ```
    #[inline]
    fn default() -> Self {
        Self::for_domain(Domain::General)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Default Implementation Tests
    // =========================================================================

    #[test]
    fn test_default_is_general() {
        let domain = Domain::default();
        assert_eq!(domain, Domain::General, "Default domain must be General");
    }

    // =========================================================================
    // Description Method Tests
    // =========================================================================

    #[test]
    fn test_description_non_empty_for_all_variants() {
        for domain in Domain::all() {
            let desc = domain.description();
            assert!(!desc.is_empty(), "Description for {:?} must not be empty", domain);
            assert!(desc.len() > 10, "Description for {:?} should be meaningful", domain);
        }
    }

    #[test]
    fn test_code_description_mentions_precision() {
        assert!(Domain::Code.description().to_lowercase().contains("precision"));
    }

    #[test]
    fn test_legal_description_mentions_reasoning() {
        assert!(Domain::Legal.description().to_lowercase().contains("reasoning"));
    }

    #[test]
    fn test_medical_description_mentions_causal() {
        assert!(Domain::Medical.description().to_lowercase().contains("causal"));
    }

    #[test]
    fn test_creative_description_mentions_exploration() {
        assert!(Domain::Creative.description().to_lowercase().contains("exploration"));
    }

    #[test]
    fn test_research_description_mentions_balanced() {
        assert!(Domain::Research.description().to_lowercase().contains("balanced"));
    }

    #[test]
    fn test_general_description_mentions_default() {
        assert!(Domain::General.description().to_lowercase().contains("default"));
    }

    // =========================================================================
    // all() Method Tests
    // =========================================================================

    #[test]
    fn test_all_returns_6_variants() {
        let all = Domain::all();
        assert_eq!(all.len(), 6, "Domain::all() must return exactly 6 variants");
    }

    #[test]
    fn test_all_contains_all_variants() {
        let all = Domain::all();
        assert!(all.contains(&Domain::Code));
        assert!(all.contains(&Domain::Legal));
        assert!(all.contains(&Domain::Medical));
        assert!(all.contains(&Domain::Creative));
        assert!(all.contains(&Domain::Research));
        assert!(all.contains(&Domain::General));
    }

    #[test]
    fn test_all_order_matches_definition() {
        let all = Domain::all();
        assert_eq!(all[0], Domain::Code);
        assert_eq!(all[1], Domain::Legal);
        assert_eq!(all[2], Domain::Medical);
        assert_eq!(all[3], Domain::Creative);
        assert_eq!(all[4], Domain::Research);
        assert_eq!(all[5], Domain::General);
    }

    // =========================================================================
    // Display Trait Tests
    // =========================================================================

    #[test]
    fn test_display_code() {
        assert_eq!(Domain::Code.to_string(), "code");
    }

    #[test]
    fn test_display_legal() {
        assert_eq!(Domain::Legal.to_string(), "legal");
    }

    #[test]
    fn test_display_medical() {
        assert_eq!(Domain::Medical.to_string(), "medical");
    }

    #[test]
    fn test_display_creative() {
        assert_eq!(Domain::Creative.to_string(), "creative");
    }

    #[test]
    fn test_display_research() {
        assert_eq!(Domain::Research.to_string(), "research");
    }

    #[test]
    fn test_display_general() {
        assert_eq!(Domain::General.to_string(), "general");
    }

    #[test]
    fn test_display_all_lowercase() {
        for domain in Domain::all() {
            let s = domain.to_string();
            assert_eq!(s, s.to_lowercase(), "Display for {:?} must be lowercase", domain);
        }
    }

    // =========================================================================
    // Serde Serialization Tests
    // =========================================================================

    #[test]
    fn test_serde_serializes_to_lowercase() {
        let domain = Domain::Code;
        let json = serde_json::to_string(&domain).expect("serialize failed");
        assert_eq!(json, r#""code""#, "Serde must serialize to lowercase");
    }

    #[test]
    fn test_serde_deserializes_from_lowercase() {
        let domain: Domain = serde_json::from_str(r#""legal""#).expect("deserialize failed");
        assert_eq!(domain, Domain::Legal);
    }

    #[test]
    fn test_serde_roundtrip_all_variants() {
        for domain in Domain::all() {
            let json = serde_json::to_string(&domain).expect("serialize failed");
            let restored: Domain = serde_json::from_str(&json).expect("deserialize failed");
            assert_eq!(restored, domain, "Serde roundtrip failed for {:?}", domain);
        }
    }

    #[test]
    fn test_serde_snake_case_format() {
        // All variants should serialize to snake_case (which for single words is just lowercase)
        for domain in Domain::all() {
            let json = serde_json::to_string(&domain).unwrap();
            // Remove quotes
            let value = json.trim_matches('"');
            assert!(value.chars().all(|c| c.is_lowercase() || c == '_'),
                "Serde output for {:?} must be snake_case: {}", domain, value);
        }
    }

    // =========================================================================
    // Derive Trait Tests
    // =========================================================================

    #[test]
    fn test_clone() {
        let domain = Domain::Medical;
        let cloned = domain.clone();
        assert_eq!(domain, cloned);
    }

    #[test]
    fn test_copy() {
        let domain = Domain::Creative;
        let copied = domain; // Copy, not move
        assert_eq!(domain, copied);
        let _still_valid = domain; // Can still use original
    }

    #[test]
    fn test_debug_format() {
        let debug = format!("{:?}", Domain::Research);
        assert!(debug.contains("Research"), "Debug should show variant name");
    }

    #[test]
    fn test_partial_eq() {
        assert_eq!(Domain::Code, Domain::Code);
        assert_ne!(Domain::Code, Domain::Legal);
    }

    #[test]
    fn test_hash_in_collection() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Domain::Code);
        set.insert(Domain::Legal);
        set.insert(Domain::Code); // Duplicate
        assert_eq!(set.len(), 2, "Hash must properly deduplicate");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_all_variants_unique() {
        use std::collections::HashSet;
        let all = Domain::all();
        let unique: HashSet<_> = all.iter().collect();
        assert_eq!(unique.len(), 6, "All variants must be unique");
    }

    #[test]
    fn test_default_is_in_all() {
        let default = Domain::default();
        assert!(Domain::all().contains(&default), "Default must be in all()");
    }

    // =========================================================================
    // NeurotransmitterWeights Tests
    // =========================================================================

    // --- Constructor Tests ---

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

    // --- for_domain() Tests ---

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
            assert!(weights.validate(), "Domain {:?} produced invalid weights", domain);
        }
    }

    // --- compute_effective_weight() Tests ---

    #[test]
    fn test_nt_compute_effective_weight_general_base_1() {
        let weights = NeurotransmitterWeights::for_domain(Domain::General);
        // General: (1.0*0.5 - 1.0*0.2) * (1 + (0.3-0.5)*0.4) = 0.3 * 0.92 = 0.276
        let effective = weights.compute_effective_weight(1.0);
        assert!((effective - 0.276).abs() < 0.001, "Expected ~0.276, got {}", effective);
    }

    #[test]
    fn test_nt_compute_effective_weight_creative_amplifies() {
        let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
        // Creative has high excitatory (0.8), low inhibitory (0.1)
        // (1.0*0.8 - 1.0*0.1) * (1 + (0.6-0.5)*0.4) = 0.7 * 1.04 = 0.728
        let effective = weights.compute_effective_weight(1.0);
        assert!((effective - 0.728).abs() < 0.001, "Expected ~0.728, got {}", effective);
    }

    #[test]
    fn test_nt_compute_effective_weight_legal_dampens() {
        let weights = NeurotransmitterWeights::for_domain(Domain::Legal);
        // Legal has equal excitatory/inhibitory (0.4, 0.4) = net zero signal
        // (1.0*0.4 - 1.0*0.4) * (1 + (0.2-0.5)*0.4) = 0.0 * 0.88 = 0.0
        let effective = weights.compute_effective_weight(1.0);
        assert!((effective - 0.0).abs() < 0.001, "Expected ~0.0, got {}", effective);
    }

    #[test]
    fn test_nt_compute_effective_weight_clamps_high() {
        // Create weights that would produce > 1.0 before clamping
        let weights = NeurotransmitterWeights::new(1.0, 0.0, 1.0);
        // (1.0*1.0 - 1.0*0.0) * (1 + (1.0-0.5)*0.4) = 1.0 * 1.2 = 1.2 -> clamp to 1.0
        let effective = weights.compute_effective_weight(1.0);
        assert_eq!(effective, 1.0, "Must clamp to 1.0, got {}", effective);
    }

    #[test]
    fn test_nt_compute_effective_weight_clamps_low() {
        // Create weights that would produce < 0.0 before clamping
        let weights = NeurotransmitterWeights::new(0.0, 1.0, 0.0);
        // (1.0*0.0 - 1.0*1.0) * (1 + (0.0-0.5)*0.4) = -1.0 * 0.8 = -0.8 -> clamp to 0.0
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
        // Test many combinations to ensure output is always [0.0, 1.0]
        for exc in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for inh in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for modul in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    for base in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        let weights = NeurotransmitterWeights::new(exc, inh, modul);
                        let effective = weights.compute_effective_weight(base);
                        assert!(
                            effective >= 0.0 && effective <= 1.0,
                            "Out of range: exc={}, inh={}, mod={}, base={} -> {}",
                            exc, inh, modul, base, effective
                        );
                    }
                }
            }
        }
    }

    // --- validate() Tests ---

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
        assert!(!weights.validate(), "Infinity must fail validation per AP-009");
    }

    #[test]
    fn test_nt_validate_neg_infinity() {
        let weights = NeurotransmitterWeights::new(f32::NEG_INFINITY, 0.5, 0.5);
        assert!(!weights.validate(), "Neg infinity must fail validation per AP-009");
    }

    // --- Default Implementation Tests ---

    #[test]
    fn test_nt_default_is_general() {
        let default_weights = NeurotransmitterWeights::default();
        let general_weights = NeurotransmitterWeights::for_domain(Domain::General);
        assert_eq!(default_weights, general_weights, "Default must equal General profile");
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

    // --- Derive Trait Tests ---

    #[test]
    fn test_nt_clone() {
        let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
        let cloned = weights.clone();
        assert_eq!(weights, cloned);
    }

    #[test]
    fn test_nt_copy() {
        let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
        let copied = weights; // Copy, not move
        assert_eq!(weights, copied);
        let _still_valid = weights; // Can still use original
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

    // --- Serde Tests ---

    #[test]
    fn test_nt_serde_roundtrip() {
        let weights = NeurotransmitterWeights::new(0.6, 0.3, 0.4);
        let json = serde_json::to_string(&weights).expect("serialize failed");
        let restored: NeurotransmitterWeights = serde_json::from_str(&json).expect("deserialize failed");
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
            let restored: NeurotransmitterWeights = serde_json::from_str(&json).expect("deserialize failed");
            assert_eq!(weights, restored, "Roundtrip failed for {:?}", domain);
        }
    }
}
