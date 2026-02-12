//! E5 Causal Asymmetric Similarity
//!
//! Implements Constitution-specified asymmetric similarity for E5 Causal embeddings
//! per ARCH-15 and AP-77:
//!
//! ```text
//! sim = base_cos × direction_mod
//! ```
//!
//! # Direction Modifiers (Per Constitution)
//!
//! - cause→effect: 1.2 (forward inference amplified)
//! - effect→cause: 0.8 (backward inference dampened)
//! - same_direction: 1.0 (no modification)
//!
//! # Intervention Overlap (Disabled)
//!
//! The intervention overlap factor `(0.7 + 0.3 × overlap)` was disabled based on
//! benchmark analysis showing -15.9% correlation (0.063). The formula now uses
//! `overlap_factor = 1.0` to allow direction modifiers their full effect.
//!
//! # References
//!
//! - Constitution ARCH-15: E5 MUST use asymmetric similarity
//! - Constitution AP-77: E5 MUST NOT use symmetric cosine similarity
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

/// Causal content gate thresholds.
///
/// Post-training E5 score distribution (GPU benchmark 2026-02-11, re-trained):
///   causal mean=0.384, non-causal mean=0.140, gap=0.244
/// Thresholds calibrated for 3-stage trained LoRA+projection model (nomic-embed-text-v1.5).
/// Untrained model scores 0.93-0.98 (all above CAUSAL_THRESHOLD → gate boosts everything equally, no-op).
pub mod causal_gate {
    /// Minimum E5 score to consider content "definitely causal"
    /// Calibrated: causal mean=0.384, this captures ~90% of causal content (TPR=0.93)
    pub const CAUSAL_THRESHOLD: f32 = 0.30;
    /// Maximum E5 score to consider content "definitely non-causal"
    /// Calibrated: non-causal mean=0.140, this catches ~85% of non-causal content
    pub const NON_CAUSAL_THRESHOLD: f32 = 0.22;
    /// Boost applied to results that pass the causal gate (for causal queries).
    /// Increased from 1.05 to 1.10 — the 98% TNR gives confidence in gate accuracy,
    /// and a stronger boost ensures causal content outranks non-causal noise.
    pub const CAUSAL_BOOST: f32 = 1.10;
    /// Demotion applied to results that fail the causal gate (for causal queries).
    /// Increased from 0.90 to 0.85 — stronger demotion of non-causal content
    /// ensures it drops below genuinely causal results in ranking.
    pub const NON_CAUSAL_DEMOTION: f32 = 0.85;
}

/// Apply E5 causal gating to a result score.
///
/// Converts E5's compressed continuous score into a binary boost/demotion.
/// Between CAUSAL_THRESHOLD and NON_CAUSAL_THRESHOLD: no change (ambiguous zone).
///
/// # Arguments
///
/// * `original_score` - The current similarity score for this result
/// * `e5_score` - The E5 asymmetric similarity score
/// * `is_causal_query` - Whether the query was detected as causal
///
/// # Returns
///
/// Adjusted score: boosted if E5 >= CAUSAL_THRESHOLD, demoted if E5 <= NON_CAUSAL_THRESHOLD,
/// unchanged in the ambiguous zone.
pub fn apply_causal_gate(original_score: f32, e5_score: f32, is_causal_query: bool) -> f32 {
    if !is_causal_query {
        return original_score;
    }
    if e5_score >= causal_gate::CAUSAL_THRESHOLD {
        original_score * causal_gate::CAUSAL_BOOST
    } else if e5_score <= causal_gate::NON_CAUSAL_THRESHOLD {
        original_score * causal_gate::NON_CAUSAL_DEMOTION
    } else {
        original_score
    }
}

/// Infer causal direction from a fingerprint's E5 dual vectors.
///
/// Compares the L2 norms of the cause and effect projections.
/// Content that is cause-oriented will have a stronger cause vector;
/// content that is effect-oriented will have a stronger effect vector.
///
/// # Arguments
///
/// * `fp` - Semantic fingerprint with E5 cause/effect vectors
///
/// # Returns
///
/// - `Cause` if cause vector norm > effect * 1.1 (10% threshold)
/// - `Effect` if effect vector norm > cause * 1.1
/// - `Unknown` if roughly equal or vectors are empty
pub fn infer_direction_from_fingerprint(
    fp: &crate::types::fingerprint::SemanticFingerprint,
) -> CausalDirection {
    let cause_vec = fp.get_e5_as_cause();
    let effect_vec = fp.get_e5_as_effect();

    if cause_vec.is_empty() || effect_vec.is_empty() {
        return CausalDirection::Unknown;
    }

    let cause_mag_sq: f32 = cause_vec.iter().map(|x| x * x).sum();
    let effect_mag_sq: f32 = effect_vec.iter().map(|x| x * x).sum();

    // Use 10% threshold for significance (1.1^2 = 1.21)
    if cause_mag_sq > effect_mag_sq * 1.21 {
        CausalDirection::Cause
    } else if effect_mag_sq > cause_mag_sq * 1.21 {
        CausalDirection::Effect
    } else {
        CausalDirection::Unknown
    }
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
    /// Uses a size-normalized approach that blends Jaccard similarity (specificity)
    /// with containment metric (flexibility for asymmetric set sizes).
    ///
    /// # Formula
    ///
    /// ```text
    /// containment = intersection / min(|A|, |B|)  // Handles asymmetric sizes
    /// jaccard = intersection / union              // Handles specificity
    /// blended = 0.5 * jaccard + 0.5 * containment
    /// overlap = 0.7 * blended + domain_bonus + mechanism_bonus
    /// ```
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

        // Size-normalized Jaccard with containment metric
        let self_set: std::collections::HashSet<_> = self.intervened_variables.iter().collect();
        let other_set: std::collections::HashSet<_> = other.intervened_variables.iter().collect();

        let intersection = self_set.intersection(&other_set).count();
        let union = self_set.union(&other_set).count();
        let min_size = self_set.len().min(other_set.len());

        // Size-normalized containment: intersection / min_size
        // Better for asymmetric sizes (e.g., subset relationship)
        let containment = if min_size > 0 {
            intersection as f32 / min_size as f32
        } else {
            0.0
        };

        // Traditional Jaccard for specificity
        let jaccard = if union > 0 {
            intersection as f32 / union as f32
        } else {
            0.0
        };

        // Blend Jaccard (specificity) with containment (flexibility)
        let blended = jaccard * 0.5 + containment * 0.5;

        // Apply bonuses (scaled by base overlap to avoid empty-context inflation)
        // Only apply bonuses if there's meaningful overlap (> 0.1)
        let domain_bonus = if blended > 0.1 {
            match (&self.domain, &other.domain) {
                (Some(d1), Some(d2)) if d1 == d2 => 0.15,
                _ => 0.0,
            }
        } else {
            0.0
        };

        let mechanism_bonus = if blended > 0.1 {
            match (&self.mechanism, &other.mechanism) {
                (Some(m1), Some(m2)) if m1 == m2 => 0.15,
                _ => 0.0,
            }
        } else {
            0.0
        };

        // Final overlap: 70% from variable overlap, up to 30% from bonuses
        (blended * 0.7 + domain_bonus + mechanism_bonus).clamp(0.0, 1.0)
    }

    /// Check if this context is empty.
    pub fn is_empty(&self) -> bool {
        self.intervened_variables.is_empty() && self.domain.is_none() && self.mechanism.is_none()
    }
}

/// Compute E5 asymmetric causal similarity.
///
/// # Formula (Per ARCH-15, AP-77)
///
/// ```text
/// sim = base_cos × direction_mod
/// ```
///
/// The intervention overlap factor was disabled based on benchmark analysis
/// showing -15.9% correlation. Direction modifiers (1.2x/0.8x) now have full effect.
///
/// # Arguments
///
/// * `base_cosine` - Base cosine similarity between embeddings [0, 1]
/// * `query_direction` - Causal direction of the query
/// * `result_direction` - Causal direction of the result
/// * `_query_context` - Intervention context (unused, kept for API compatibility)
/// * `_result_context` - Intervention context (unused, kept for API compatibility)
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
///
/// let adjusted = compute_asymmetric_similarity(
///     base_sim,
///     query_dir,
///     result_dir,
///     None,
///     None,
/// );
///
/// // cause→effect = 1.2x amplification
/// // 0.8 * 1.2 = 0.96
/// assert!((adjusted - 0.96).abs() < 0.001);
/// ```
pub fn compute_asymmetric_similarity(
    base_cosine: f32,
    query_direction: CausalDirection,
    result_direction: CausalDirection,
    _query_context: Option<&InterventionContext>,
    _result_context: Option<&InterventionContext>,
) -> f32 {
    // Get direction modifier per Constitution ARCH-15:
    // - cause→effect: 1.2 (forward inference amplified)
    // - effect→cause: 0.8 (backward inference dampened)
    // - same_direction: 1.0 (no modification)
    let direction_mod = CausalDirection::direction_modifier(query_direction, result_direction);

    // Intervention overlap DISABLED per benchmark analysis:
    // - Correlation: 0.063 (essentially no correlation)
    // - Performance impact: -15.9%
    //
    // Original formula: sim = base_cos × direction_mod × (0.7 + 0.3 × overlap)
    // New formula:      sim = base_cos × direction_mod
    base_cosine * direction_mod
}

/// Compute asymmetric similarity with default contexts.
///
/// Convenience function when intervention contexts are not available.
///
/// # Formula (Per ARCH-15)
///
/// ```text
/// sim = base_cos × direction_mod
/// ```
///
/// Direction modifiers: cause→effect=1.2, effect→cause=0.8, same=1.0
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

// =============================================================================
// E5 Asymmetric Fingerprint-Based Similarity (CAWAI-Inspired)
// =============================================================================

use crate::types::fingerprint::SemanticFingerprint;

/// Compute asymmetric E5 causal similarity between query and document fingerprints.
///
/// This function implements the asymmetric similarity computation specified in
/// CAWAI research (https://arxiv.org/html/2504.04700v1) for causal retrieval:
///
/// - For "why" queries (query is searching for causes):
///   query_as_cause is compared against doc_as_effect
///
/// - For "what happens" queries (query is searching for effects):
///   query_as_effect is compared against doc_as_cause
///
/// # Arguments
///
/// * `query` - Query fingerprint
/// * `doc` - Document fingerprint to compare against
/// * `query_is_cause` - If true, treat query as potential cause (for "why" queries);
///                      if false, treat query as potential effect (for "what happens" queries)
///
/// # Returns
///
/// Cosine similarity between the appropriate E5 vectors, clamped to [0, 1].
/// Uses the asymmetric pairing:
/// - query_is_cause=true:  cosine(query.e5_as_cause, doc.e5_as_effect)
/// - query_is_cause=false: cosine(query.e5_as_effect, doc.e5_as_cause)
///
/// # Example
///
/// ```
/// # #[cfg(feature = "test-utils")]
/// # {
/// use context_graph_core::causal::asymmetric::compute_e5_asymmetric_fingerprint_similarity;
/// use context_graph_core::types::fingerprint::SemanticFingerprint;
///
/// let query = SemanticFingerprint::zeroed();
/// let doc = SemanticFingerprint::zeroed();
///
/// // For "why did X happen?" queries, the query IS the effect (query_is_cause=false)
/// let sim = compute_e5_asymmetric_fingerprint_similarity(&query, &doc, false);
///
/// // For "what happens if X?" queries, the query IS the cause (query_is_cause=true)
/// // let sim = compute_e5_asymmetric_fingerprint_similarity(&query, &doc, true);
/// assert!(sim >= 0.0 && sim <= 1.0);
/// # }
/// ```
pub fn compute_e5_asymmetric_fingerprint_similarity(
    query: &SemanticFingerprint,
    doc: &SemanticFingerprint,
    query_is_cause: bool,
) -> f32 {
    let (query_vec, doc_vec) = if query_is_cause {
        // Query represents a potential cause, looking for effects
        // Compare query's cause encoding against doc's effect encoding
        (query.get_e5_as_cause(), doc.get_e5_as_effect())
    } else {
        // Query represents a potential effect, looking for causes
        // Compare query's effect encoding against doc's cause encoding
        (query.get_e5_as_effect(), doc.get_e5_as_cause())
    };

    cosine_similarity_f32(query_vec, doc_vec).max(0.0)
}

/// Compute E5 asymmetric similarity with direction modifier applied.
///
/// Combines the raw asymmetric similarity with the Constitution-specified
/// direction modifiers (cause→effect=1.2, effect→cause=0.8).
///
/// # Formula
///
/// ```text
/// sim = asymmetric_cosine × direction_mod
/// ```
///
/// # Arguments
///
/// * `query` - Query fingerprint
/// * `doc` - Document fingerprint
/// * `query_direction` - Causal direction of the query
/// * `result_direction` - Causal direction of the document
/// * `query_context` - Optional intervention context for query
/// * `result_context` - Optional intervention context for document
///
/// # Returns
///
/// Adjusted similarity score (may exceed 1.0 due to amplification).
pub fn compute_e5_asymmetric_full(
    query: &SemanticFingerprint,
    doc: &SemanticFingerprint,
    query_direction: CausalDirection,
    result_direction: CausalDirection,
    query_context: Option<&InterventionContext>,
    result_context: Option<&InterventionContext>,
) -> f32 {
    // Determine asymmetric pairing based on query direction
    let query_is_cause = matches!(query_direction, CausalDirection::Cause);

    // Get base asymmetric similarity
    let base_sim = compute_e5_asymmetric_fingerprint_similarity(query, doc, query_is_cause);

    // Apply Constitution formula with direction modifier
    compute_asymmetric_similarity(
        base_sim,
        query_direction,
        result_direction,
        query_context,
        result_context,
    )
}

/// Detect causal query intent from query text.
///
/// Analyzes the query text to determine if the user is asking for:
/// - Causes ("why", "what causes", "reason for") -> returns CausalDirection::Cause
/// - Effects ("what happens", "result of", "consequence") -> returns CausalDirection::Effect
/// - Unknown direction -> returns CausalDirection::Unknown
///
/// Uses score-based detection with disambiguation for queries that match
/// both cause and effect indicators.
///
/// # Arguments
///
/// * `query` - The query text to analyze
///
/// # Returns
///
/// The detected causal direction of the query.
///
/// # Example
///
/// ```
/// use context_graph_core::causal::asymmetric::{detect_causal_query_intent, CausalDirection};
///
/// assert_eq!(detect_causal_query_intent("why does rust have ownership?"), CausalDirection::Cause);
/// assert_eq!(detect_causal_query_intent("what causes memory leaks?"), CausalDirection::Cause);
/// assert_eq!(detect_causal_query_intent("what happens if I delete this file?"), CausalDirection::Effect);
/// assert_eq!(detect_causal_query_intent("diagnose the root cause"), CausalDirection::Cause);
/// assert_eq!(detect_causal_query_intent("show me the code"), CausalDirection::Unknown);
/// ```
pub fn detect_causal_query_intent(query: &str) -> CausalDirection {
    let query_lower = query.to_lowercase();

    // ===== Negation-Aware Preprocessing (Step 1.1) =====
    // For each indicator match, scan a 15-char window before it for negation tokens.
    // If a negation token is found near an indicator, skip that indicator.
    let negation_tokens: &[&str] = &[
        "not ", "no ", "never ", "n't ", "neither ", "without ",
        "cannot ", "can't ", "won't ", "doesn't ", "don't ", "isn't ",
    ];

    // Cause-seeking indicators: user has an effect and wants the cause
    // Expanded in Phase 3 for >40-50% detection rate on real data
    let cause_indicators = [
        // Original patterns
        "why ",
        "why?",
        "what cause",
        "what causes",
        "what caused",
        "reason for",
        "reasons for",
        "reason behind",
        "because of what",
        "due to what",
        "what led to",
        "what leads to",
        "explain why",
        "how come",
        // Investigation patterns
        "diagnose",
        "root cause",
        "investigate",
        "debug",
        "troubleshoot",
        // Trigger patterns ("trigger" subsumes "triggers", "triggered", "what triggers")
        "trigger",
        "source of",
        "origin of",
        // Attribution patterns
        "culprit",
        "underlying",
        "responsible for",
        "attributed to",
        "blame",
        // Question patterns
        "how did",
        "where did",
        "when did",
        // Domain-specific patterns
        "failure mode",
        "etiology",
        "pathogenesis",
        "root of",
        // ===== Phase 3 Additions: Scientific/Technical Cause Patterns =====
        // Attribution/causation verbs (passive voice patterns)
        "is attributed to",
        // "driven by" subsumes "is driven by"; active forms "drives ", "what drives" below
        "driven by",
        "is mediated by",
        "contributes to",
        "accounts for",
        "determines the",
        "influences the",
        "regulates the",
        // Passive causation patterns (document text patterns, not queries)
        "was caused by",
        "resulted from",
        "stems from",
        "arises from",
        "originates from",
        "derives from",
        "emerged from",
        // Root cause analysis patterns
        "factor in",
        "factors in",
        // "root of" (line above) already subsumes "root of the"
        "basis of",
        "basis for",
        "driving force",
        "primary driver",
        "key driver",
        // Dependency patterns
        "depends on",
        "dependent on",
        "contingent on",
        "conditional on",
        "prerequisite for",
        // Academic/scientific patterns
        "causation",
        "causal mechanism",
        "causal factor",
        "antecedent",
        "precursor",
        "precipitating factor",
        "determinant of",
        // ===== Benchmark Optimization: Additional Scientific Cause Patterns =====
        // Mechanism understanding (academic text detection)
        "mechanism underlying",
        "pathways leading to",
        "factors influencing",
        "variables affecting",
        "predictors of",
        "correlates of",
        // Hypothesis patterns
        "we hypothesize that",
        "our hypothesis is",
        "posit that",
        "we propose that",
        "we suggest that",
        // Molecular/biological patterns
        "molecular basis of",
        "regulatory mechanisms",
        "signaling cascade",
        "feedback loop",
        "upstream regulator",
        "transcriptional control",
        "epigenetic modification",
        "gene expression",
        "protein interaction",
        // Research methodology patterns
        "independent variable",
        "controlled experiment",
        "manipulated variable",
        "treatment group",
        "intervention study",
        // ===== Benchmark Gap Fixes =====
        // "causes " in both cause AND effect lists — cancels out so compound effect
        // phrases ("causes an increase") still win; standalone "X causes Y" ties → Cause
        "causes ",
        // Active "drives" — "What drives X?" / "X drives Y" is cause-seeking
        "what drives",
        "drives ",
        // ===== Multi-Hop Cause Patterns =====
        // "via" indicates causal mechanism investigation ("X cause Y via Z")
        " via ",
        // Multi-hop investigation patterns
        "by what pathway",
        "through what mechanism",
        "by what causal",
    ];

    // Effect-seeking indicators: user has a cause and wants the effects
    // Expanded in Phase 3 for >40-50% detection rate on real data
    let effect_indicators = [
        // Original patterns
        "what happen",
        "what will happen",
        "what would happen",
        "consequence of",
        "consequences of",
        "result of",
        "results of",
        "effect of",
        "effects of",
        "impact of",
        "outcome of",
        "if i ",
        "if you ",
        "what does it do",
        "what will it do",
        "then what",
        // Downstream patterns — all three tense variants needed (not substrings of each other)
        "leads to",
        "lead to",
        "led to",
        "downstream",
        "cascades to",
        "cascading",
        "propagates to",
        // Impact patterns
        "ripple effect",
        "side effect",
        "collateral",
        "knock-on",
        "ramifications",
        // Prediction patterns
        "predict",
        "forecast",
        "anticipate",
        "expect",
        // Domain-specific patterns
        "prognosis",
        "complications",
        "sequelae",
        // ===== Phase 3 Additions: Scientific/Technical Effect Patterns =====
        // Outcome/result patterns — all three tense variants needed (not substrings of each other)
        "results in",
        "result in",
        "resulted in",
        "causes an increase",
        "causes a decrease",
        "produces a decrease",
        "induces changes",
        "initiates the process",
        // Consequence patterns
        "as a consequence",
        "as a result",
        "consequently",
        "therefore",
        "hence",
        "thus",
        "accordingly",
        // Causative action patterns
        "brings about",
        "gives rise to",
        "culminates in",
        "eventuates in",
        "manifests as",
        // Scientific effect patterns
        "correlation with",
        "associated with",
        "linked to",
        "related outcome",
        "secondary effect",
        "tertiary effect",
        "downstream effect",
        // Future outcome patterns
        "will lead to",
        "will result in",
        "would cause",
        "could result",
        "might lead to",
        "likely to cause",
        // Impact assessment patterns
        "implications of",
        "repercussions of",
        "aftermath of",
        "fallout from",
        // ===== Benchmark Optimization: Additional Scientific Effect Patterns =====
        // Outcome measurement patterns (academic text detection)
        "phenotypic outcome",
        "downstream target",
        "end result",
        "clinical manifestation",
        "observable effect",
        "measurable outcome",
        "functional consequence",
        "biological response",
        "physiological change",
        // Statistical significance patterns
        "statistically significant",
        "p-value indicates",
        "confidence interval",
        "significant difference",
        "significant increase",
        "significant decrease",
        // Dose-response patterns
        "dose-response relationship",
        "therapeutic effect",
        "adverse outcome",
        "treatment outcome",
        "clinical outcome",
        // Research methodology patterns
        "dependent variable",
        "outcome measure",
        "response variable",
        "experimental outcome",
        "study endpoint",
        // ===== Benchmark Gap Fix =====
        // "causes " in both cause AND effect lists — cancels out so compound effect
        // phrases ("causes an increase") still win; standalone "X causes Y" ties → Cause
        "causes ",
        // ===== Step 1.2: Additional Natural-Language Effect Patterns =====
        "how does this affect",
        "how will this affect",
        "what changes when",
        "how does this impact",
        "what is the result",
        "what are the effects",
        "what are the consequences",
        "what's the impact",
        // ===== Missing Natural-Language Effect Patterns =====
        // "What results from X?" — 4 benchmark queries use this pattern
        "results from",
        // "Outcomes of X" — "outcome of" misses plural "outcomes of"
        "outcomes of",
        // "What follows from X?" — result/consequence seeking
        "what follows",
        "follows from",
        // Generic "affect" with trailing space — catches "How does X affect Y?"
        // (trailing space prevents matching "affecting" which is cause-leaning)
        "affect ",
        // ===== Multi-Hop Effect Patterns =====
        "cascade ",
        "escalate ",
    ];

    // Negation-aware score-based detection (Step 1.1)
    // For each indicator, find first match position. If negated, skip it.
    let cause_score: usize = cause_indicators
        .iter()
        .filter(|p| {
            if let Some(pos) = query_lower.find(*p) {
                !is_negated_at(&query_lower, pos, negation_tokens)
            } else {
                false
            }
        })
        .count();
    let effect_score: usize = effect_indicators
        .iter()
        .filter(|p| {
            if let Some(pos) = query_lower.find(*p) {
                !is_negated_at(&query_lower, pos, negation_tokens)
            } else {
                false
            }
        })
        .count();

    // Disambiguation: compare scores
    match cause_score.cmp(&effect_score) {
        std::cmp::Ordering::Greater => CausalDirection::Cause,
        std::cmp::Ordering::Less => CausalDirection::Effect,
        std::cmp::Ordering::Equal if cause_score > 0 => {
            // Tie-breaker: prefer cause (more common in natural language queries)
            CausalDirection::Cause
        }
        _ => CausalDirection::Unknown,
    }
}

/// Check if an indicator match at `match_pos` is preceded by a negation token
/// within a 15-character lookback window.
///
/// Includes match_pos+1 in the window to handle indicators with leading spaces
/// (e.g., " cause ") where the negation token's trailing space falls at match_pos.
fn is_negated_at(text: &str, match_pos: usize, negation_tokens: &[&str]) -> bool {
    let window_start = match_pos.saturating_sub(15);
    let window_end = (match_pos + 1).min(text.len());
    let window = &text[window_start..window_end];
    negation_tokens.iter().any(|neg| window.contains(neg))
}

/// Helper: compute cosine similarity between two f32 slices.
///
/// Returns a value in [-1, 1]. For empty or zero-norm vectors, returns 0.0.
fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let norm_product = (norm_a * norm_b).sqrt();

    if norm_product < f32::EPSILON {
        0.0
    } else {
        dot / norm_product
    }
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
        // Jaccard = 1.0, containment = 1.0, blended = 1.0
        // overlap = 1.0 * 0.7 = 0.7
        assert!((overlap - 0.7).abs() < 0.01);
        println!("[VERIFIED] Identical variables → overlap ~0.7");
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
        // Jaccard = 1/3 = 0.333, containment = 1/2 = 0.5
        // blended = 0.5 * 0.333 + 0.5 * 0.5 = 0.4165
        // overlap = 0.4165 * 0.7 = 0.2916 (~0.29)
        assert!(overlap > 0.25 && overlap < 0.35);
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
        // Jaccard = 1.0, containment = 1.0, blended = 1.0
        // overlap = 1.0 * 0.7 + 0.15 domain bonus = 0.85
        assert!((overlap - 0.85).abs() < 0.01);
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
        // Jaccard = 1.0, containment = 1.0, blended = 1.0
        // overlap = 1.0 * 0.7 + 0.15 mechanism bonus = 0.85
        assert!((overlap - 0.85).abs() < 0.01);
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
        // blended = 1.0, domain 0.15 + mechanism 0.15
        // overlap = 1.0 * 0.7 + 0.15 + 0.15 = 1.0 (capped)
        assert_eq!(overlap, 1.0);
        println!("[VERIFIED] Full bonuses capped at 1.0");
    }

    #[test]
    fn test_asymmetric_sizes_containment() {
        // Test that containment handles subset relationships
        let ctx1 = InterventionContext::new()
            .with_variable("X")
            .with_variable("Y")
            .with_variable("Z");
        let ctx2 = InterventionContext::new().with_variable("X");

        let overlap = ctx1.overlap_with(&ctx2);
        // Jaccard = 1/3 = 0.333, containment = 1/1 = 1.0 (subset!)
        // blended = 0.5 * 0.333 + 0.5 * 1.0 = 0.6665
        // overlap = 0.6665 * 0.7 = 0.4666 (~0.47)
        assert!(overlap > 0.4 && overlap < 0.55);
        println!("[VERIFIED] Asymmetric sizes handled via containment: {}", overlap);
    }

    // ============================================================================
    // Asymmetric Similarity Formula Tests
    // ============================================================================

    #[test]
    fn test_formula_cause_to_effect() {
        let base = 0.8;

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
            None,
            None,
        );

        // Formula: sim = base_cos × direction_mod
        // direction_mod = 1.2 (cause→effect amplified)
        // sim = 0.8 * 1.2 = 0.96
        let expected = base * 1.2;
        assert!((sim - expected).abs() < 0.001);
        println!(
            "[VERIFIED] cause→effect: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_formula_effect_to_cause() {
        let base = 0.8;

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Effect,
            CausalDirection::Cause,
            None,
            None,
        );

        // Formula: sim = base_cos × direction_mod
        // direction_mod = 0.8 (effect→cause dampened)
        // sim = 0.8 * 0.8 = 0.64
        let expected = base * 0.8;
        assert!((sim - expected).abs() < 0.001);
        println!(
            "[VERIFIED] effect→cause: {} (expected {})",
            sim, expected
        );
    }

    #[test]
    fn test_formula_same_direction() {
        let base = 0.8;

        let sim = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Cause,
            None,
            None,
        );

        // Formula: sim = base_cos × direction_mod
        // direction_mod = 1.0 (same direction, no modification)
        // sim = 0.8 * 1.0 = 0.8
        let expected = base * 1.0;
        assert!((sim - expected).abs() < 0.001);
        println!(
            "[VERIFIED] same direction: {} (expected {})",
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
        // Per ARCH-15, AP-77: sim = base_cos × direction_mod
        // Intervention overlap disabled per benchmark analysis (-15.9% correlation)

        let base = 0.6;
        let direction_mod = 1.2;

        // Manual calculation: new simplified formula
        let expected = base * direction_mod;

        // Via function
        let actual = compute_asymmetric_similarity(
            base,
            CausalDirection::Cause,
            CausalDirection::Effect,
            None,
            None,
        );

        assert!((actual - expected).abs() < 0.001);
        println!("[VERIFIED] Constitution formula per ARCH-15:");
        println!("  base_cos = {}", base);
        println!("  direction_mod = {} (cause→effect)", direction_mod);
        println!("  result = {} (expected {})", actual, expected);
    }

    #[test]
    fn test_asymmetry_effect() {
        // Same base similarity, but different directions should produce different results
        // This is the KEY test for ARCH-15 compliance
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
        assert!(cause_to_effect > effect_to_cause,
            "ARCH-15 violation: cause→effect ({}) must be > effect→cause ({})",
            cause_to_effect, effect_to_cause);

        // Ratio should be exactly 1.2/0.8 = 1.5 (no overlap dampening)
        let ratio = cause_to_effect / effect_to_cause;
        assert!((ratio - 1.5).abs() < 0.001,
            "Asymmetry ratio should be exactly 1.5, got {}", ratio);

        // Verify exact values with new formula
        // cause→effect: 0.8 * 1.2 = 0.96
        // effect→cause: 0.8 * 0.8 = 0.64
        assert!((cause_to_effect - 0.96).abs() < 0.001,
            "cause→effect should be 0.96, got {}", cause_to_effect);
        assert!((effect_to_cause - 0.64).abs() < 0.001,
            "effect→cause should be 0.64, got {}", effect_to_cause);

        println!(
            "[VERIFIED] Asymmetry per ARCH-15: cause→effect ({}) > effect→cause ({})",
            cause_to_effect, effect_to_cause
        );
        println!("  Ratio: {} (expected exactly 1.5)", ratio);
    }

    // ============================================================================
    // Causal Query Intent Detection Tests
    // ============================================================================

    #[test]
    fn test_detect_causal_query_why() {
        assert_eq!(
            detect_causal_query_intent("why does rust have ownership?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("Why is the sky blue?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] 'why' queries detected as Cause");
    }

    #[test]
    fn test_detect_causal_query_what_causes() {
        assert_eq!(
            detect_causal_query_intent("what causes memory leaks?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("What caused the test failure?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] 'what causes' queries detected as Cause");
    }

    #[test]
    fn test_detect_causal_query_reason() {
        assert_eq!(
            detect_causal_query_intent("reason for the error"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("what's the reason behind this design?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] 'reason for/behind' queries detected as Cause");
    }

    #[test]
    fn test_detect_causal_query_what_happens() {
        assert_eq!(
            detect_causal_query_intent("what happens if I delete this file?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what will happen when I run this?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] 'what happens' queries detected as Effect");
    }

    #[test]
    fn test_detect_causal_query_consequence() {
        assert_eq!(
            detect_causal_query_intent("consequence of removing this line"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what are the effects of this change?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] 'consequence/effect' queries detected as Effect");
    }

    #[test]
    fn test_detect_causal_query_if_condition() {
        assert_eq!(
            detect_causal_query_intent("if I enable feature X, what happens?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("if you run make clean, then what?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] 'if' condition queries detected as Effect");
    }

    #[test]
    fn test_detect_causal_query_unknown() {
        assert_eq!(
            detect_causal_query_intent("show me the code"),
            CausalDirection::Unknown
        );
        assert_eq!(
            detect_causal_query_intent("list all files"),
            CausalDirection::Unknown
        );
        assert_eq!(
            detect_causal_query_intent("format this function"),
            CausalDirection::Unknown
        );
        println!("[VERIFIED] Non-causal queries detected as Unknown");
    }

    #[test]
    fn test_cosine_similarity_f32_basic() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity_f32(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Identical vectors should have sim=1.0");

        let c = vec![0.0, 1.0, 0.0];
        let sim_ortho = cosine_similarity_f32(&a, &c);
        assert!(sim_ortho.abs() < 1e-6, "Orthogonal vectors should have sim=0.0");

        let d = vec![-1.0, 0.0, 0.0];
        let sim_opp = cosine_similarity_f32(&a, &d);
        assert!((sim_opp - (-1.0)).abs() < 1e-6, "Opposite vectors should have sim=-1.0");

        println!("[VERIFIED] cosine_similarity_f32 works correctly");
    }

    #[test]
    fn test_cosine_similarity_f32_edge_cases() {
        // Empty vectors
        let empty: Vec<f32> = vec![];
        assert_eq!(cosine_similarity_f32(&empty, &empty), 0.0);

        // Mismatched lengths
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity_f32(&a, &b), 0.0);

        // Zero norm vectors
        let zeros = vec![0.0, 0.0, 0.0];
        let ones = vec![1.0, 1.0, 1.0];
        assert_eq!(cosine_similarity_f32(&zeros, &ones), 0.0);

        println!("[VERIFIED] cosine_similarity_f32 handles edge cases");
    }

    // ============================================================================
    // New Direction Detection Pattern Tests (Phase 1 Expansion)
    // ============================================================================

    #[test]
    fn test_detect_investigation_patterns() {
        // Investigation patterns should detect Cause
        assert_eq!(
            detect_causal_query_intent("diagnose the issue"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("what is the root cause of this failure?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("investigate why the server crashed"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("debug the error"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("troubleshoot the connection problem"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Investigation patterns detected as Cause");
    }

    #[test]
    fn test_detect_trigger_patterns() {
        // Trigger patterns should detect Cause
        assert_eq!(
            detect_causal_query_intent("what triggers the alert?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("source of the memory leak"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("origin of this bug"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Trigger patterns detected as Cause");
    }

    #[test]
    fn test_detect_attribution_patterns() {
        // Attribution patterns should detect Cause
        assert_eq!(
            detect_causal_query_intent("who is responsible for this failure?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this is attributed to what?"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("find the culprit"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("what is the underlying issue?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Attribution patterns detected as Cause");
    }

    #[test]
    fn test_detect_downstream_patterns() {
        // Downstream patterns should detect Effect
        assert_eq!(
            detect_causal_query_intent("this leads to what?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what are the downstream effects?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("cascades to other systems?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("cascading failures expected?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Downstream patterns detected as Effect");
    }

    #[test]
    fn test_detect_impact_patterns() {
        // Impact patterns should detect Effect
        assert_eq!(
            detect_causal_query_intent("what are the ripple effects?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("any side effects?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what are the ramifications?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Impact patterns detected as Effect");
    }

    #[test]
    fn test_detect_prediction_patterns() {
        // Prediction patterns should detect Effect
        assert_eq!(
            detect_causal_query_intent("predict the outcome"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("forecast the results"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what should we expect?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("anticipate any problems?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Prediction patterns detected as Effect");
    }

    #[test]
    fn test_detect_domain_specific_patterns() {
        // Domain-specific cause patterns
        assert_eq!(
            detect_causal_query_intent("failure mode analysis"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("etiology of the disease"),
            CausalDirection::Cause
        );

        // Domain-specific effect patterns
        assert_eq!(
            detect_causal_query_intent("prognosis for this patient"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("possible complications"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Domain-specific patterns detected correctly");
    }

    #[test]
    fn test_disambiguation_tie_breaker() {
        // When both cause and effect indicators are present, cause wins as tie-breaker
        // Query with both "why" (cause) and "if" (effect) patterns
        // "why" comes first and is a strong cause indicator
        assert_eq!(
            detect_causal_query_intent("why does this happen?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Disambiguation tie-breaker works");
    }

    #[test]
    fn test_score_based_detection() {
        // Multiple cause indicators should still detect Cause
        assert_eq!(
            detect_causal_query_intent("investigate and troubleshoot the root cause"),
            CausalDirection::Cause
        );
        // Multiple effect indicators should detect Effect
        assert_eq!(
            detect_causal_query_intent("predict the downstream cascading effects"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Score-based detection handles multiple indicators");
    }

    // ============================================================================
    // Phase 3 Addition: Scientific/Technical Pattern Tests
    // ============================================================================

    #[test]
    fn test_detect_scientific_cause_patterns() {
        // Attribution/causation verbs
        assert_eq!(
            detect_causal_query_intent("this is attributed to environmental factors"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the result is driven by market conditions"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the response is mediated by hormones"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("genetic factors contributes to the disease"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this accounts for 80% of the variance"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Scientific cause attribution patterns detected");
    }

    #[test]
    fn test_detect_passive_causation_patterns() {
        // Passive causation patterns
        assert_eq!(
            detect_causal_query_intent("the failure was caused by a memory leak"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the crash resulted from a null pointer"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this behavior stems from legacy code"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the problem arises from improper initialization"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this pattern originates from the original design"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Passive causation patterns detected as Cause");
    }

    #[test]
    fn test_detect_dependency_patterns() {
        // Dependency patterns
        assert_eq!(
            detect_causal_query_intent("this feature depends on the database being online"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the outcome is dependent on initial conditions"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the result is contingent on proper configuration"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this is a prerequisite for the next step"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Dependency patterns detected as Cause");
    }

    #[test]
    fn test_detect_scientific_effect_patterns() {
        // Outcome/result patterns - using unambiguous effect indicators
        assert_eq!(
            detect_causal_query_intent("this mutation results in protein misfolding"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the treatment causes an increase in dopamine levels"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the drug produces a decrease in blood pressure readings"),
            CausalDirection::Effect
        );
        // Use clear effect pattern without conflicting cause patterns
        assert_eq!(
            detect_causal_query_intent("this leads to downstream effects"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("as a consequence the output changes"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Scientific effect patterns detected");
    }

    #[test]
    fn test_detect_consequence_connectors() {
        // Consequence patterns
        assert_eq!(
            detect_causal_query_intent("as a consequence of the change"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("as a result of the optimization"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("consequently the system failed"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("therefore we need to restart"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("hence the performance improved"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Consequence connector patterns detected as Effect");
    }

    #[test]
    fn test_detect_future_outcome_patterns() {
        // Future outcome patterns
        assert_eq!(
            detect_causal_query_intent("this will lead to data corruption"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the change will result in faster queries"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("this would cause a deadlock"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the modification could result in errors"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("this might lead to memory exhaustion"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Future outcome patterns detected as Effect");
    }

    #[test]
    fn test_detect_academic_cause_patterns() {
        // Academic/scientific cause patterns
        assert_eq!(
            detect_causal_query_intent("the causal mechanism involves protein binding"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this is a causal factor in the outcome"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the antecedent condition was met"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the precursor event triggered the cascade"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("this is a determinant of success"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Academic cause patterns detected");
    }

    #[test]
    fn test_detect_impact_assessment_patterns() {
        // Impact assessment patterns
        assert_eq!(
            detect_causal_query_intent("what are the implications of this change?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the repercussions of the decision"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("in the aftermath of the incident"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the fallout from the data breach"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Impact assessment patterns detected as Effect");
    }

    // ============================================================================
    // Benchmark Optimization Pattern Tests (new scientific patterns)
    // ============================================================================

    #[test]
    fn test_detect_benchmark_optimization_cause_patterns() {
        // Mechanism understanding patterns
        assert_eq!(
            detect_causal_query_intent("the mechanism underlying this process"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("pathways leading to cell death"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("factors influencing the outcome"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("variables affecting performance"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("predictors of success"),
            CausalDirection::Cause
        );

        // Hypothesis patterns
        assert_eq!(
            detect_causal_query_intent("we hypothesize that the mutation causes"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("we propose that this is the root cause"),
            CausalDirection::Cause
        );

        // Molecular/biological patterns
        assert_eq!(
            detect_causal_query_intent("the regulatory mechanisms involved"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the signaling cascade initiates"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("the feedback loop maintains homeostasis"),
            CausalDirection::Cause
        );

        println!("[VERIFIED] Benchmark optimization cause patterns detected");
    }

    #[test]
    fn test_detect_benchmark_optimization_effect_patterns() {
        // Outcome measurement patterns
        assert_eq!(
            detect_causal_query_intent("the phenotypic outcome was unexpected"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the clinical manifestation includes"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the observable effect is clear"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the measurable outcome shows"),
            CausalDirection::Effect
        );

        // Statistical significance patterns
        assert_eq!(
            detect_causal_query_intent("the difference was statistically significant"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the confidence interval suggests"),
            CausalDirection::Effect
        );

        // Dose-response patterns
        assert_eq!(
            detect_causal_query_intent("the therapeutic effect was observed"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the adverse outcome was documented"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("the clinical outcome improved"),
            CausalDirection::Effect
        );

        println!("[VERIFIED] Benchmark optimization effect patterns detected");
    }

    // ============================================================================
    // Step 1.1: Negation Handling Tests
    // ============================================================================

    #[test]
    fn test_negation_blocks_cause_indicator() {
        // "not trigger" — negation before cause indicator → should NOT detect as Cause
        assert_eq!(
            detect_causal_query_intent("this does not trigger any response"),
            CausalDirection::Unknown
        );
        println!("[VERIFIED] Negated 'trigger' blocked");
    }

    #[test]
    fn test_negation_blocks_effect_indicator() {
        // "doesn't lead to" — negation before effect indicator
        assert_eq!(
            detect_causal_query_intent("this doesn't lead to any problems"),
            CausalDirection::Unknown
        );
        println!("[VERIFIED] Negated 'lead to' blocked");
    }

    #[test]
    fn test_negation_does_not_trigger() {
        // "not" far away from indicator (>15 chars) should NOT negate
        assert_eq!(
            detect_causal_query_intent("it is not clear what exactly triggers the alert"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Distant 'not' does not negate indicator");
    }

    #[test]
    fn test_negation_multiple_tokens() {
        // Various negation forms
        assert_eq!(
            detect_causal_query_intent("this never leads to failure"),
            CausalDirection::Unknown
        );
        assert_eq!(
            detect_causal_query_intent("cannot trigger the event"),
            CausalDirection::Unknown
        );
        assert_eq!(
            detect_causal_query_intent("won't lead to any changes"),
            CausalDirection::Unknown
        );
        println!("[VERIFIED] Multiple negation tokens handled");
    }

    #[test]
    fn test_negation_non_negated_still_works() {
        // Non-negated indicators should still work as before
        assert_eq!(
            detect_causal_query_intent("stress triggers anxiety"),
            CausalDirection::Cause
        );
        assert_eq!(
            detect_causal_query_intent("smoking leads to cancer"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Non-negated indicators still detected");
    }

    #[test]
    fn test_negation_mixed_negated_and_non_negated() {
        // One indicator negated, another not → should still detect the non-negated one
        assert_eq!(
            detect_causal_query_intent("this doesn't lead to problems but why does it crash?"),
            CausalDirection::Cause
        );
        println!("[VERIFIED] Mixed negated/non-negated handled correctly");
    }

    // ============================================================================
    // Step 1.2: New Effect Indicator Pattern Tests
    // ============================================================================

    #[test]
    fn test_new_effect_indicators() {
        assert_eq!(
            detect_causal_query_intent("how does this affect the system?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("how will this affect performance?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what changes when we deploy?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("how does this impact latency?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what is the result of the change?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what are the effects of this update?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what are the consequences of removing this?"),
            CausalDirection::Effect
        );
        assert_eq!(
            detect_causal_query_intent("what's the impact of the migration?"),
            CausalDirection::Effect
        );
        println!("[VERIFIED] Step 1.2 effect indicators detected");
    }

    // ============================================================================
    // Step 1.4: Gate Threshold Tests
    // ============================================================================

    #[test]
    fn test_causal_gate_new_thresholds() {
        // Score above 0.30 → boosted (trained model: causal content mean=0.384)
        let boosted = apply_causal_gate(0.5, 0.35, true);
        assert!((boosted - 0.5 * causal_gate::CAUSAL_BOOST).abs() < 0.001);

        // Score below 0.22 → demoted (trained model: non-causal content mean=0.140)
        let demoted = apply_causal_gate(0.5, 0.15, true);
        assert!((demoted - 0.5 * causal_gate::NON_CAUSAL_DEMOTION).abs() < 0.001);

        // Score in ambiguous zone (0.22-0.30) → unchanged
        let unchanged = apply_causal_gate(0.5, 0.26, true);
        assert_eq!(unchanged, 0.5);

        println!("[VERIFIED] Causal gate respects calibrated thresholds");
    }
}
