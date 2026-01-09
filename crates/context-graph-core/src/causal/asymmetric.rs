//! E5 Causal Asymmetric Similarity
//!
//! Implements Constitution-specified asymmetric similarity for E5 Causal embeddings:
//!
//! ```text
//! sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
//! ```
//!
//! # Direction Modifiers (Per Constitution)
//!
//! - cause→effect: 1.2 (forward inference amplified)
//! - effect→cause: 0.8 (backward inference dampened)
//! - same_direction: 1.0 (no modification)
//!
//! # References
//!
//! - Constitution `causal_asymmetric_sim` section
//! - PRD Section 11.2: E5 Causal embedding asymmetric similarity

use serde::{Deserialize, Serialize};

use super::inference::InferenceDirection;

/// Direction modifiers per Constitution specification.
///
/// # Constitution Reference
/// ```yaml
/// causal_asymmetric_sim:
///   direction_modifiers:
///     cause_to_effect: 1.2
///     effect_to_cause: 0.8
///     same_direction: 1.0
/// ```
pub mod direction_mod {
    /// cause→effect amplification factor
    pub const CAUSE_TO_EFFECT: f32 = 1.2;
    /// effect→cause dampening factor
    pub const EFFECT_TO_CAUSE: f32 = 0.8;
    /// No modification for same-direction comparisons
    pub const SAME_DIRECTION: f32 = 1.0;
    /// Default for unknown direction (no modification)
    pub const UNKNOWN: f32 = 1.0;
}

/// Causal direction for asymmetric similarity computation.
///
/// Simplified direction enum specifically for similarity computation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CausalDirection {
    /// Entity is a cause (produces effects)
    Cause,
    /// Entity is an effect (produced by causes)
    Effect,
    /// Direction unknown or bidirectional
    #[default]
    Unknown,
}

impl CausalDirection {
    /// Convert from InferenceDirection.
    pub fn from_inference_direction(dir: InferenceDirection) -> Self {
        match dir {
            InferenceDirection::Forward => Self::Cause, // Forward = we're the cause
            InferenceDirection::Backward => Self::Effect, // Backward = we're looking for causes
            InferenceDirection::Bidirectional => Self::Unknown,
            InferenceDirection::Bridge => Self::Unknown,
            InferenceDirection::Abduction => Self::Effect, // Looking for cause of observation
        }
    }

    /// Get direction modifier when comparing query_direction to result_direction.
    ///
    /// # Returns
    ///
    /// Direction modifier per Constitution:
    /// - 1.2 if query=Cause and result=Effect (cause→effect)
    /// - 0.8 if query=Effect and result=Cause (effect→cause)
    /// - 1.0 otherwise (same direction or unknown)
    pub fn direction_modifier(query_direction: Self, result_direction: Self) -> f32 {
        match (query_direction, result_direction) {
            // Query is cause looking for effect: AMPLIFY
            (Self::Cause, Self::Effect) => direction_mod::CAUSE_TO_EFFECT,
            // Query is effect looking for cause: DAMPEN
            (Self::Effect, Self::Cause) => direction_mod::EFFECT_TO_CAUSE,
            // Same direction or unknown: NO CHANGE
            (Self::Cause, Self::Cause) => direction_mod::SAME_DIRECTION,
            (Self::Effect, Self::Effect) => direction_mod::SAME_DIRECTION,
            (Self::Unknown, _) => direction_mod::UNKNOWN,
            (_, Self::Unknown) => direction_mod::UNKNOWN,
        }
    }
}

impl std::fmt::Display for CausalDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cause => write!(f, "cause"),
            Self::Effect => write!(f, "effect"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Intervention context for computing intervention overlap.
///
/// Represents the interventional variables involved in a causal analysis.
/// Used to compute the intervention_overlap term in the asymmetric similarity formula.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InterventionContext {
    /// Names/IDs of variables that are intervened upon
    pub intervened_variables: Vec<String>,
    /// Domain of the intervention (e.g., "physics", "economics")
    pub domain: Option<String>,
    /// Mechanism being targeted by intervention
    pub mechanism: Option<String>,
}

impl InterventionContext {
    /// Create a new empty intervention context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an intervened variable.
    pub fn with_variable(mut self, var: impl Into<String>) -> Self {
        self.intervened_variables.push(var.into());
        self
    }

    /// Set the domain.
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set the mechanism.
    pub fn with_mechanism(mut self, mechanism: impl Into<String>) -> Self {
        self.mechanism = Some(mechanism.into());
        self
    }

    /// Compute intervention overlap with another context.
    ///
    /// Overlap is computed as Jaccard similarity of intervened variables,
    /// with bonuses for matching domain and mechanism.
    ///
    /// # Returns
    ///
    /// Value in [0, 1] where:
    /// - 0 = no shared interventions
    /// - 1 = perfect overlap in variables, domain, and mechanism
    pub fn overlap_with(&self, other: &Self) -> f32 {
        if self.intervened_variables.is_empty() && other.intervened_variables.is_empty() {
            // Both empty contexts: treat as neutral (0.5)
            return 0.5;
        }

        if self.intervened_variables.is_empty() || other.intervened_variables.is_empty() {
            // One empty, one not: minimal overlap
            return 0.1;
        }

        // Jaccard similarity for variables
        let self_set: std::collections::HashSet<_> = self.intervened_variables.iter().collect();
        let other_set: std::collections::HashSet<_> = other.intervened_variables.iter().collect();

        let intersection = self_set.intersection(&other_set).count();
        let union = self_set.union(&other_set).count();

        let jaccard = if union > 0 {
            intersection as f32 / union as f32
        } else {
            0.0
        };

        // Domain bonus (0.1 if matching)
        let domain_bonus = match (&self.domain, &other.domain) {
            (Some(d1), Some(d2)) if d1 == d2 => 0.1,
            _ => 0.0,
        };

        // Mechanism bonus (0.1 if matching)
        let mechanism_bonus = match (&self.mechanism, &other.mechanism) {
            (Some(m1), Some(m2)) if m1 == m2 => 0.1,
            _ => 0.0,
        };

        // Final overlap clamped to [0, 1]
        (jaccard * 0.8 + domain_bonus + mechanism_bonus).clamp(0.0, 1.0)
    }

    /// Check if this context is empty.
    pub fn is_empty(&self) -> bool {
        self.intervened_variables.is_empty() && self.domain.is_none() && self.mechanism.is_none()
    }
}

/// Compute E5 asymmetric causal similarity.
///
/// # Formula (Constitution)
///
/// ```text
/// sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
/// ```
///
/// # Arguments
///
/// * `base_cosine` - Base cosine similarity between embeddings [0, 1]
/// * `query_direction` - Causal direction of the query
/// * `result_direction` - Causal direction of the result
/// * `query_context` - Intervention context of the query (optional)
/// * `result_context` - Intervention context of the result (optional)
///
/// # Returns
///
/// Adjusted similarity value. Note: Can exceed 1.0 due to direction_mod=1.2.
///
/// # Example
///
/// ```
/// use context_graph_core::causal::asymmetric::{
///     compute_asymmetric_similarity, CausalDirection, InterventionContext
/// };
///
/// let base_sim = 0.8;
/// let query_dir = CausalDirection::Cause;
/// let result_dir = CausalDirection::Effect;
/// let query_ctx = InterventionContext::new().with_variable("temperature");
/// let result_ctx = InterventionContext::new().with_variable("temperature");
///
/// let adjusted = compute_asymmetric_similarity(
///     base_sim,
///     query_dir,
///     result_dir,
///     Some(&query_ctx),
///     Some(&result_ctx),
/// );
///
/// // cause→effect with high overlap = amplified similarity
/// assert!(adjusted > base_sim);
/// ```
pub fn compute_asymmetric_similarity(
    base_cosine: f32,
    query_direction: CausalDirection,
    result_direction: CausalDirection,
    query_context: Option<&InterventionContext>,
    result_context: Option<&InterventionContext>,
) -> f32 {
    // Get direction modifier
    let direction_mod = CausalDirection::direction_modifier(query_direction, result_direction);

    // Compute intervention overlap
    let intervention_overlap = match (query_context, result_context) {
        (Some(q), Some(r)) => q.overlap_with(r),
        _ => 0.5, // Default to neutral if no context provided
    };

    // Apply Constitution formula:
    // sim = base_cos × direction_mod × (0.7 + 0.3 × intervention_overlap)
    let overlap_factor = 0.7 + 0.3 * intervention_overlap;

    base_cosine * direction_mod * overlap_factor
}

/// Compute asymmetric similarity with default (neutral) contexts.
///
/// Convenience function when intervention contexts are not available.
///
/// # Formula (Simplified)
///
/// ```text
/// sim = base_cos × direction_mod × 0.85
/// ```
///
/// (0.85 = 0.7 + 0.3 × 0.5 for neutral overlap)
pub fn compute_asymmetric_similarity_simple(
    base_cosine: f32,
    query_direction: CausalDirection,
    result_direction: CausalDirection,
) -> f32 {
    compute_asymmetric_similarity(base_cosine, query_direction, result_direction, None, None)
}

/// Adjust a batch of similarity scores with the same query context.
///
/// Optimized for multi-result scenarios where the query is constant.
///
/// # Arguments
///
/// * `base_similarities` - Slice of (base_cosine, result_direction, result_context) tuples
/// * `query_direction` - Causal direction of the query
/// * `query_context` - Intervention context of the query (optional)
///
/// # Returns
///
/// Vector of adjusted similarities in the same order as input.
pub fn adjust_batch_similarities(
    base_similarities: &[(f32, CausalDirection, Option<&InterventionContext>)],
    query_direction: CausalDirection,
    query_context: Option<&InterventionContext>,
) -> Vec<f32> {
    base_similarities
        .iter()
        .map(|(base, result_dir, result_ctx)| {
            compute_asymmetric_similarity(
                *base,
                query_direction,
                *result_dir,
                query_context,
                *result_ctx,
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================================
    // Direction Modifier Tests
    // ============================================================================

    #[test]
    fn test_direction_mod_cause_to_effect() {
        let modifier =
            CausalDirection::direction_modifier(CausalDirection::Cause, CausalDirection::Effect);
        assert_eq!(modifier, 1.2);
        println!("[VERIFIED] cause→effect direction_mod = 1.2");
    }

    #[test]
    fn test_direction_mod_effect_to_cause() {
        let modifier =
            CausalDirection::direction_modifier(CausalDirection::Effect, CausalDirection::Cause);
        assert_eq!(modifier, 0.8);
        println!("[VERIFIED] effect→cause direction_mod = 0.8");
    }

    #[test]
    fn test_direction_mod_same_direction() {
        assert_eq!(
            CausalDirection::direction_modifier(CausalDirection::Cause, CausalDirection::Cause),
            1.0
        );
        assert_eq!(
            CausalDirection::direction_modifier(CausalDirection::Effect, CausalDirection::Effect),
            1.0
        );
        println!("[VERIFIED] same_direction direction_mod = 1.0");
    }

    #[test]
    fn test_direction_mod_unknown() {
        assert_eq!(
            CausalDirection::direction_modifier(CausalDirection::Unknown, CausalDirection::Cause),
            1.0
        );
        assert_eq!(
            CausalDirection::direction_modifier(CausalDirection::Effect, CausalDirection::Unknown),
            1.0
        );
        println!("[VERIFIED] unknown direction_mod = 1.0");
    }

    // ============================================================================
    // Intervention Context Tests
    // ============================================================================

    #[test]
    fn test_empty_contexts_neutral_overlap() {
        let ctx1 = InterventionContext::new();
        let ctx2 = InterventionContext::new();

        let overlap = ctx1.overlap_with(&ctx2);
        assert_eq!(overlap, 0.5);
        println!("[VERIFIED] Empty contexts → neutral overlap 0.5");
    }

    #[test]
    fn test_one_empty_context_minimal_overlap() {
        let ctx1 = InterventionContext::new().with_variable("X");
        let ctx2 = InterventionContext::new();

        let overlap = ctx1.overlap_with(&ctx2);
        assert_eq!(overlap, 0.1);
        println!("[VERIFIED] One empty context → minimal overlap 0.1");
    }

    #[test]
    fn test_identical_variables_high_overlap() {
        let ctx1 = InterventionContext::new()
            .with_variable("temperature")
            .with_variable("pressure");
        let ctx2 = InterventionContext::new()
            .with_variable("temperature")
            .with_variable("pressure");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard = 1.0, scaled by 0.8 = 0.8
        assert!((overlap - 0.8).abs() < 0.01);
        println!("[VERIFIED] Identical variables → overlap ~0.8");
    }

    #[test]
    fn test_partial_overlap() {
        let ctx1 = InterventionContext::new()
            .with_variable("temperature")
            .with_variable("pressure");
        let ctx2 = InterventionContext::new()
            .with_variable("temperature")
            .with_variable("volume");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard = 1/3 = 0.333, scaled by 0.8 = 0.266
        assert!(overlap > 0.2 && overlap < 0.3);
        println!("[VERIFIED] Partial overlap computed correctly: {}", overlap);
    }

    #[test]
    fn test_domain_bonus() {
        let ctx1 = InterventionContext::new()
            .with_variable("X")
            .with_domain("physics");
        let ctx2 = InterventionContext::new()
            .with_variable("X")
            .with_domain("physics");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard = 1.0 * 0.8 + 0.1 domain bonus = 0.9
        assert!((overlap - 0.9).abs() < 0.01);
        println!("[VERIFIED] Domain bonus applied: {}", overlap);
    }

    #[test]
    fn test_mechanism_bonus() {
        let ctx1 = InterventionContext::new()
            .with_variable("X")
            .with_mechanism("heat_transfer");
        let ctx2 = InterventionContext::new()
            .with_variable("X")
            .with_mechanism("heat_transfer");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard = 1.0 * 0.8 + 0.1 mechanism bonus = 0.9
        assert!((overlap - 0.9).abs() < 0.01);
        println!("[VERIFIED] Mechanism bonus applied: {}", overlap);
    }

    #[test]
    fn test_full_bonuses_capped_at_1() {
        let ctx1 = InterventionContext::new()
            .with_variable("X")
            .with_domain("physics")
            .with_mechanism("heat_transfer");
        let ctx2 = InterventionContext::new()
            .with_variable("X")
            .with_domain("physics")
            .with_mechanism("heat_transfer");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard * 0.8 + domain 0.1 + mechanism 0.1 = 1.0 (capped)
        assert_eq!(overlap, 1.0);
        println!("[VERIFIED] Full bonuses capped at 1.0");
    }

    // ============================================================================
    // Asymmetric Similarity Formula Tests
    // ============================================================================

    #[test]
    fn test_formula_cause_to_effect_high_overlap() {
        let base = 0.8;
        let query_ctx = InterventionContext::new().with_variable("X");
        let result_ctx = InterventionContext::new().with_variable("X");

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
            Some(&query_ctx),
            Some(&result_ctx),
        );

        // direction_mod = 1.2, overlap = 0.8
        // factor = 0.7 + 0.3 * 0.8 = 0.94
        // sim = 0.8 * 1.2 * 0.94 = 0.9024
        let expected = base * 1.2 * (0.7 + 0.3 * 0.8);
        assert!((sim - expected).abs() < 0.01);
        println!(
            "[VERIFIED] cause→effect with high overlap: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_formula_effect_to_cause_high_overlap() {
        let base = 0.8;
        let query_ctx = InterventionContext::new().with_variable("X");
        let result_ctx = InterventionContext::new().with_variable("X");

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Effect,
            CausalDirection::Cause,
            Some(&query_ctx),
            Some(&result_ctx),
        );

        // direction_mod = 0.8, overlap = 0.8
        // factor = 0.7 + 0.3 * 0.8 = 0.94
        // sim = 0.8 * 0.8 * 0.94 = 0.6016
        let expected = base * 0.8 * (0.7 + 0.3 * 0.8);
        assert!((sim - expected).abs() < 0.01);
        println!(
            "[VERIFIED] effect→cause with high overlap: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_formula_no_context() {
        let base = 0.8;

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
            None,
            None,
        );

        // direction_mod = 1.2, overlap = 0.5 (default)
        // factor = 0.7 + 0.3 * 0.5 = 0.85
        // sim = 0.8 * 1.2 * 0.85 = 0.816
        let expected = base * 1.2 * 0.85;
        assert!((sim - expected).abs() < 0.01);
        println!(
            "[VERIFIED] cause→effect no context: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_simple_function_matches() {
        let base = 0.8;
        let query_dir = CausalDirection::Cause;
        let result_dir = CausalDirection::Effect;

        let full = compute_asymmetric_similarity(base, query_dir, result_dir, None, None);
        let simple = compute_asymmetric_similarity_simple(base, query_dir, result_dir);

        assert_eq!(full, simple);
        println!("[VERIFIED] Simple function matches full with None contexts");
    }

    #[test]
    fn test_batch_adjustment() {
        let query_dir = CausalDirection::Cause;
        let query_ctx = InterventionContext::new().with_variable("X");

        let result_ctx1 = InterventionContext::new().with_variable("X");
        let result_ctx2 = InterventionContext::new().with_variable("Y");

        let batch = vec![
            (0.8, CausalDirection::Effect, Some(&result_ctx1)),
            (0.7, CausalDirection::Effect, Some(&result_ctx2)),
            (0.9, CausalDirection::Cause, None),
        ];

        let adjusted = adjust_batch_similarities(&batch, query_dir, Some(&query_ctx));

        assert_eq!(adjusted.len(), 3);
        // First: cause→effect with high overlap → highest adjustment
        // Second: cause→effect with low overlap → lower adjustment
        // Third: cause→cause with neutral overlap → moderate adjustment
        assert!(adjusted[0] > adjusted[1]);
        println!("[VERIFIED] Batch adjustment produces {:?}", adjusted);
    }

    // ============================================================================
    // Constitution Compliance Tests
    // ============================================================================

    #[test]
    fn test_constitution_direction_mod_values() {
        // Constitution: cause_to_effect: 1.2
        assert_eq!(direction_mod::CAUSE_TO_EFFECT, 1.2);
        // Constitution: effect_to_cause: 0.8
        assert_eq!(direction_mod::EFFECT_TO_CAUSE, 0.8);
        // Constitution: same_direction: 1.0
        assert_eq!(direction_mod::SAME_DIRECTION, 1.0);

        println!("[VERIFIED] All direction_mod values match Constitution spec");
    }

    #[test]
    fn test_constitution_formula_components() {
        // Constitution formula: sim = base_cos × direction_mod × (0.7 + 0.3×intervention_overlap)

        let base = 0.6;
        let direction_mod = 1.2;
        let intervention_overlap = 0.5;

        // Manual calculation
        let expected = base * direction_mod * (0.7 + 0.3 * intervention_overlap);

        // Via function (neutral overlap = 0.5)
        let actual = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
            None,
            None,
        );

        assert!((actual - expected).abs() < 0.01);
        println!("[VERIFIED] Constitution formula implemented correctly");
        println!("  base_cos = {}", base);
        println!("  direction_mod = {} (cause→effect)", direction_mod);
        println!(
            "  intervention_overlap = {} (neutral default)",
            intervention_overlap
        );
        println!("  result = {} (expected {})", actual, expected);
    }

    #[test]
    fn test_asymmetry_effect() {
        // Same base similarity, but different directions should produce different results
        let base = 0.8;

        let cause_to_effect = compute_asymmetric_similarity_simple(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
        );

        let effect_to_cause = compute_asymmetric_similarity_simple(
            base,
            CausalDirection::Effect,
            CausalDirection::Cause,
        );

        // cause→effect should be HIGHER than effect→cause
        assert!(cause_to_effect > effect_to_cause);

        // Ratio should be 1.2/0.8 = 1.5
        let ratio = cause_to_effect / effect_to_cause;
        assert!((ratio - 1.5).abs() < 0.01);

        println!(
            "[VERIFIED] Asymmetry: cause→effect ({}) > effect→cause ({})",
            cause_to_effect, effect_to_cause
        );
        println!("  Ratio: {} (expected 1.5)", ratio);
    }
}
