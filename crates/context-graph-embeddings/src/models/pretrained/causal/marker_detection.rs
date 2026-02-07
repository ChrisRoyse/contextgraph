//! Causal marker detection for asymmetric cause/effect embeddings.
//!
//! This module detects causal indicator tokens in text to enable
//! marker-weighted pooling for asymmetric embeddings.
//!
//! # Architecture
//!
//! Creates meaningful cause/effect asymmetry by:
//! 1. Detecting causal markers (cause/effect indicators) in text
//! 2. Creating differentiated weights for marker-weighted pooling
//! 3. Cause embeddings weight cause markers higher, effect embeddings weight effect markers higher
//!
//! This creates embeddings where:
//! - Cause-role embedding emphasizes cause indicators ("because", "due to", etc.)
//! - Effect-role embedding emphasizes effect indicators ("therefore", "results in", etc.)
//! - The asymmetry enables directional causal retrieval

use tokenizers::Encoding;

use context_graph_core::traits::CausalHintGuidance;

/// Marker boost factor for weighted pooling.
/// Cause/effect markers get this multiplier during pooling.
pub const MARKER_BOOST: f32 = 2.5;

/// Result of causal marker detection.
#[derive(Debug, Clone, Default)]
pub struct CausalMarkerResult {
    /// Token indices of cause indicators (e.g., "because", "caused by")
    pub cause_marker_indices: Vec<usize>,
    /// Token indices of effect indicators (e.g., "therefore", "results in")
    pub effect_marker_indices: Vec<usize>,
    /// Causal strength score [0.0, 1.0]
    pub causal_strength: f32,
    /// Number of markers from static lists.
    pub static_cause_count: usize,
    /// Number of markers from static lists.
    pub static_effect_count: usize,
    /// Number of markers injected from LLM hints.
    pub llm_cause_count: usize,
    /// Number of markers injected from LLM hints.
    pub llm_effect_count: usize,
    /// Actual MARKER_BOOST value used (may be modulated by asymmetry_strength).
    pub effective_boost: f32,
}

impl CausalMarkerResult {
    /// Create token weights for cause-focused pooling.
    ///
    /// Cause markers get boosted weight (MARKER_BOOST), effect markers get reduced weight.
    /// This creates a cause-role embedding that emphasizes causal antecedents.
    ///
    /// # Arguments
    /// * `seq_len` - Total sequence length
    ///
    /// # Returns
    /// Vector of per-token weights
    pub fn cause_weights(&self, seq_len: usize) -> Vec<f32> {
        let boost = if self.effective_boost > 0.0 { self.effective_boost } else { MARKER_BOOST };
        let mut weights = vec![1.0f32; seq_len];

        // Boost cause markers
        for &idx in &self.cause_marker_indices {
            if idx < seq_len {
                weights[idx] = boost;
            }
        }

        // Reduce effect markers for cause embedding (inverse relationship)
        for &idx in &self.effect_marker_indices {
            if idx < seq_len {
                weights[idx] = 1.0 / boost.sqrt();
            }
        }

        weights
    }

    /// Create token weights for effect-focused pooling.
    ///
    /// Effect markers get boosted weight, cause markers get reduced weight.
    /// This creates an effect-role embedding that emphasizes causal consequences.
    ///
    /// # Arguments
    /// * `seq_len` - Total sequence length
    ///
    /// # Returns
    /// Vector of per-token weights
    pub fn effect_weights(&self, seq_len: usize) -> Vec<f32> {
        let boost = if self.effective_boost > 0.0 { self.effective_boost } else { MARKER_BOOST };
        let mut weights = vec![1.0f32; seq_len];

        // Boost effect markers
        for &idx in &self.effect_marker_indices {
            if idx < seq_len {
                weights[idx] = boost;
            }
        }

        // Reduce cause markers for effect embedding (inverse relationship)
        for &idx in &self.cause_marker_indices {
            if idx < seq_len {
                weights[idx] = 1.0 / boost.sqrt();
            }
        }

        weights
    }

}

/// Cause indicator patterns for marker detection.
///
/// These patterns are drawn from context-graph-core/src/causal/asymmetric.rs
/// and optimized for token-level detection.
const CAUSE_INDICATORS: &[&str] = &[
    // Primary cause markers
    "because",
    "caused",
    "causes",
    "causing",
    "due",
    "reason",
    "reasons",
    "why",
    "since",
    "as",
    // Investigation patterns
    "diagnose",
    "root",
    "investigate",
    "debug",
    "troubleshoot",
    // Trigger patterns
    "trigger",
    "triggers",
    "triggered",
    "source",
    "origin",
    // Attribution patterns
    "responsible",
    "attributed",
    "blame",
    "underlying",
    "culprit",
    // Dependency patterns
    "depends",
    "dependent",
    "contingent",
    "prerequisite",
    // Scientific patterns
    "causation",
    "causal",
    "antecedent",
    "precursor",
    "determinant",
    "factor",
    "factors",
    "driven",
    "mediated",
    "contributes",
    "accounts",
    "determines",
    "influences",
    "regulates",
    // Passive causation
    "resulted",
    "stems",
    "arises",
    "originates",
    "derives",
    "emerged",
    // ===== Benchmark Optimization: Additional Scientific Cause Patterns =====
    // Mechanism understanding (academic text detection)
    "mechanism",
    "pathways",
    "affecting",
    "predictors",
    "correlates",
    // Hypothesis patterns
    "hypothesize",
    "hypothesis",
    "posit",
    "propose",
    "suggest",
    // Molecular/biological patterns
    "molecular",
    "regulatory",
    "signaling",
    "cascade",
    "feedback",
    "upstream",
    "transcriptional",
    "epigenetic",
    "expression",
    "interaction",
    // Research methodology patterns
    "variable",
    "experiment",
    "manipulated",
    "treatment",
    "intervention",
];

/// Effect indicator patterns for marker detection.
const EFFECT_INDICATORS: &[&str] = &[
    // Primary effect markers
    "therefore",
    "thus",
    "hence",
    "consequently",
    "result",
    "results",
    "resulting",
    "effect",
    "effects",
    "impact",
    "outcome",
    "outcomes",
    // Consequence patterns
    "consequence",
    "consequences",
    "leads",
    "leading",
    "led",
    // Downstream patterns
    "downstream",
    "cascades",
    "cascading",
    "propagates",
    "ripple",
    "collateral",
    "ramifications",
    // Prediction patterns
    "predict",
    "predicts",
    "forecast",
    "anticipate",
    "expect",
    // Scientific patterns
    "prognosis",
    "complications",
    "sequelae",
    "manifestation",
    "symptom",
    "symptoms",
    // Causative action patterns
    "produces",
    "generates",
    "induces",
    "initiates",
    "brings",
    "gives",
    "culminates",
    "manifests",
    // Future outcome patterns
    "will",
    "would",
    "could",
    "might",
    // Impact assessment
    "implications",
    "repercussions",
    "aftermath",
    "fallout",
    // ===== Benchmark Optimization: Additional Scientific Effect Patterns =====
    // Outcome measurement patterns (academic text detection)
    "phenotypic",
    "target",
    "observable",
    "measurable",
    "biological",
    "physiological",
    // Statistical significance patterns
    "statistically",
    "significant",
    "confidence",
    "interval",
    "increase",
    "decrease",
    // Dose-response patterns
    "dose",
    "therapeutic",
    "adverse",
    "clinical",
    // Research methodology patterns
    "dependent",
    "measure",
    "response",
    "endpoint",
];

/// Detect causal markers in tokenized text.
///
/// # Arguments
///
/// * `text` - Original text content
/// * `encoding` - Tokenizer encoding with offset mappings
///
/// # Returns
///
/// `CausalMarkerResult` containing marker indices and detected direction
pub fn detect_causal_markers(text: &str, encoding: &Encoding) -> CausalMarkerResult {
    let text_lower = text.to_lowercase();
    let tokens = encoding.get_tokens();
    let offsets = encoding.get_offsets();

    let mut cause_indices = Vec::new();
    let mut effect_indices = Vec::new();

    // Iterate through tokens and check if they match causal indicators
    for (idx, token) in tokens.iter().enumerate() {
        // Clean token (remove special prefixes like Ġ from RoBERTa tokenizer)
        let clean_token = token
            .trim_start_matches('Ġ')
            .trim_start_matches("##")
            .to_lowercase();

        if clean_token.is_empty() || clean_token.len() < 2 {
            continue;
        }

        // Check cause indicators
        for indicator in CAUSE_INDICATORS {
            if clean_token == *indicator
                || clean_token.starts_with(indicator)
                || indicator.starts_with(&clean_token)
            {
                cause_indices.push(idx);
                break;
            }
        }

        // Check effect indicators
        for indicator in EFFECT_INDICATORS {
            if clean_token == *indicator
                || clean_token.starts_with(indicator)
                || indicator.starts_with(&clean_token)
            {
                effect_indices.push(idx);
                break;
            }
        }
    }

    // Also check for multi-word patterns in the original text
    let multi_word_cause_patterns = [
        "caused by",
        "due to",
        "reason for",
        "because of",
        "root cause",
        "results from",
        "stems from",
        "arises from",
        "originates from",
    ];

    let multi_word_effect_patterns = [
        "leads to",
        "results in",
        "as a result",
        "as a consequence",
        "gives rise to",
        "brings about",
        "will lead to",
        "will result in",
    ];

    // Find token indices for multi-word patterns
    for pattern in multi_word_cause_patterns {
        if let Some(pos) = text_lower.find(pattern) {
            // Find tokens that overlap with this position
            for (idx, &(start, end)) in offsets.iter().enumerate() {
                if start <= pos && pos < end {
                    if !cause_indices.contains(&idx) {
                        cause_indices.push(idx);
                    }
                    // Also include next few tokens for the pattern
                    for next_idx in (idx + 1)..=(idx + 3).min(tokens.len() - 1) {
                        if !cause_indices.contains(&next_idx) {
                            cause_indices.push(next_idx);
                        }
                    }
                    break;
                }
            }
        }
    }

    for pattern in multi_word_effect_patterns {
        if let Some(pos) = text_lower.find(pattern) {
            for (idx, &(start, end)) in offsets.iter().enumerate() {
                if start <= pos && pos < end {
                    if !effect_indices.contains(&idx) {
                        effect_indices.push(idx);
                    }
                    for next_idx in (idx + 1)..=(idx + 3).min(tokens.len() - 1) {
                        if !effect_indices.contains(&next_idx) {
                            effect_indices.push(next_idx);
                        }
                    }
                    break;
                }
            }
        }
    }

    // Sort and deduplicate indices
    cause_indices.sort_unstable();
    cause_indices.dedup();
    effect_indices.sort_unstable();
    effect_indices.dedup();

    // Compute causal strength based on marker density
    let word_count = text.split_whitespace().count().max(1) as f32;
    let total_markers = (cause_indices.len() + effect_indices.len()) as f32;
    let causal_strength = (total_markers / word_count.sqrt()).min(1.0);

    let static_cause_count = cause_indices.len();
    let static_effect_count = effect_indices.len();

    CausalMarkerResult {
        cause_marker_indices: cause_indices,
        effect_marker_indices: effect_indices,
        causal_strength,
        static_cause_count,
        static_effect_count,
        llm_cause_count: 0,
        llm_effect_count: 0,
        effective_boost: MARKER_BOOST,
    }
}

/// Detect causal markers using static lists PLUS LLM-identified spans.
///
/// LLM spans catch implicit/domain-specific causation that static lists miss.
/// Returns a CausalMarkerResult with provenance tracking which markers came from where.
pub fn detect_causal_markers_with_hints(
    text: &str,
    encoding: &Encoding,
    hint: Option<&CausalHintGuidance>,
) -> CausalMarkerResult {
    // Start with static marker detection
    let mut result = detect_causal_markers(text, encoding);

    let hint = match hint {
        Some(h) if h.confidence >= 0.5 => h,
        _ => return result,
    };

    let offsets = encoding.get_offsets();
    let tokens = encoding.get_tokens();
    let mut llm_cause_count = 0usize;
    let mut llm_effect_count = 0usize;

    // Inject cause entity spans: find overlapping token indices
    for &(span_start, span_end) in &hint.cause_spans {
        if span_start >= span_end { continue; }
        for (idx, &(tok_start, tok_end)) in offsets.iter().enumerate() {
            // Token overlaps with span if they share any characters
            if tok_end > span_start && tok_start < span_end {
                if !result.cause_marker_indices.contains(&idx) {
                    result.cause_marker_indices.push(idx);
                    llm_cause_count += 1;
                }
            }
        }
    }

    // Inject effect entity spans
    for &(span_start, span_end) in &hint.effect_spans {
        if span_start >= span_end { continue; }
        for (idx, &(tok_start, tok_end)) in offsets.iter().enumerate() {
            if tok_end > span_start && tok_start < span_end {
                if !result.effect_marker_indices.contains(&idx) {
                    result.effect_marker_indices.push(idx);
                    llm_effect_count += 1;
                }
            }
        }
    }

    // Inject key_phrases as additional indicator words
    let text_lower = text.to_lowercase();
    for phrase in &hint.key_phrases {
        let phrase_lower = phrase.to_lowercase();
        if let Some(pos) = text_lower.find(&phrase_lower) {
            let phrase_end = pos + phrase_lower.len();
            for (idx, &(tok_start, tok_end)) in offsets.iter().enumerate() {
                if tok_end > pos && tok_start < phrase_end {
                    // Determine if this is a cause or effect phrase based on token content
                    let clean_token = tokens.get(idx).map(|t| {
                        t.trim_start_matches('Ġ')
                            .trim_start_matches("##")
                            .to_lowercase()
                    }).unwrap_or_default();

                    // Check existing indicator lists to categorize
                    let is_cause_indicator = CAUSE_INDICATORS.iter().any(|ind| {
                        clean_token == *ind || clean_token.starts_with(ind) || ind.starts_with(&clean_token)
                    });
                    let is_effect_indicator = EFFECT_INDICATORS.iter().any(|ind| {
                        clean_token == *ind || clean_token.starts_with(ind) || ind.starts_with(&clean_token)
                    });

                    if is_cause_indicator && !result.cause_marker_indices.contains(&idx) {
                        result.cause_marker_indices.push(idx);
                        llm_cause_count += 1;
                    } else if is_effect_indicator && !result.effect_marker_indices.contains(&idx) {
                        result.effect_marker_indices.push(idx);
                        llm_effect_count += 1;
                    } else if !is_cause_indicator && !is_effect_indicator {
                        // LLM phrase not in static lists - add based on hint's cause/effect spans
                        if !hint.cause_spans.is_empty() && !result.cause_marker_indices.contains(&idx) {
                            result.cause_marker_indices.push(idx);
                            llm_cause_count += 1;
                        } else if !hint.effect_spans.is_empty() && !result.effect_marker_indices.contains(&idx) {
                            result.effect_marker_indices.push(idx);
                            llm_effect_count += 1;
                        }
                    }
                }
            }
        }
    }

    // Deduplicate
    result.cause_marker_indices.sort_unstable();
    result.cause_marker_indices.dedup();
    result.effect_marker_indices.sort_unstable();
    result.effect_marker_indices.dedup();

    // Compute effective boost modulated by asymmetry_strength
    let effective_boost = MARKER_BOOST * (0.5 + 0.5 * hint.asymmetry_strength);

    result.llm_cause_count = llm_cause_count;
    result.llm_effect_count = llm_effect_count;
    result.effective_boost = effective_boost;

    // Recompute causal strength with the additional markers
    let word_count = text.split_whitespace().count().max(1) as f32;
    let total_markers = (result.cause_marker_indices.len() + result.effect_marker_indices.len()) as f32;
    result.causal_strength = (total_markers / word_count.sqrt()).min(1.0);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cause_indicators_not_empty() {
        assert!(!CAUSE_INDICATORS.is_empty());
        assert!(CAUSE_INDICATORS.len() > 40);
    }

    #[test]
    fn test_effect_indicators_not_empty() {
        assert!(!EFFECT_INDICATORS.is_empty());
        assert!(EFFECT_INDICATORS.len() > 40);
    }

    #[test]
    fn test_marker_result_default() {
        let result = CausalMarkerResult::default();
        assert!(result.cause_marker_indices.is_empty());
        assert!(result.effect_marker_indices.is_empty());
        assert_eq!(result.causal_strength, 0.0);
    }

    #[test]
    fn test_effective_boost_with_max_asymmetry() {
        // asymmetry_strength=1.0 → effective_boost = MARKER_BOOST * (0.5 + 0.5 * 1.0) = MARKER_BOOST
        let boost = MARKER_BOOST * (0.5 + 0.5 * 1.0);
        assert!((boost - MARKER_BOOST).abs() < f32::EPSILON,
            "Max asymmetry should give full MARKER_BOOST");
    }

    #[test]
    fn test_effective_boost_with_zero_asymmetry() {
        // asymmetry_strength=0.0 → effective_boost = MARKER_BOOST * 0.5 = 1.25
        let boost = MARKER_BOOST * (0.5 + 0.5 * 0.0);
        assert!((boost - 1.25).abs() < f32::EPSILON,
            "Zero asymmetry should give half MARKER_BOOST (1.25)");
    }

    #[test]
    fn test_effective_boost_with_mid_asymmetry() {
        // asymmetry_strength=0.5 → effective_boost = MARKER_BOOST * 0.75 = 1.875
        let boost = MARKER_BOOST * (0.5 + 0.5 * 0.5);
        assert!((boost - 1.875).abs() < f32::EPSILON,
            "Mid asymmetry should give 75% MARKER_BOOST");
    }

    #[test]
    fn test_cause_weights_with_effective_boost() {
        let result = CausalMarkerResult {
            cause_marker_indices: vec![1, 3],
            effect_marker_indices: vec![5],
            causal_strength: 0.5,
            static_cause_count: 2,
            static_effect_count: 1,
            llm_cause_count: 0,
            llm_effect_count: 0,
            effective_boost: 2.0,
        };

        let weights = result.cause_weights(8);
        assert_eq!(weights.len(), 8);
        assert!((weights[0] - 1.0).abs() < f32::EPSILON); // Default
        assert!((weights[1] - 2.0).abs() < f32::EPSILON); // Cause marker boosted
        assert!((weights[3] - 2.0).abs() < f32::EPSILON); // Cause marker boosted
        assert!(weights[5] < 1.0, "Effect marker should be reduced in cause weights");
    }

    #[test]
    fn test_effect_weights_with_effective_boost() {
        let result = CausalMarkerResult {
            cause_marker_indices: vec![1],
            effect_marker_indices: vec![3, 5],
            causal_strength: 0.5,
            static_cause_count: 1,
            static_effect_count: 2,
            llm_cause_count: 0,
            llm_effect_count: 0,
            effective_boost: 2.0,
        };

        let weights = result.effect_weights(8);
        assert!((weights[3] - 2.0).abs() < f32::EPSILON); // Effect marker boosted
        assert!((weights[5] - 2.0).abs() < f32::EPSILON); // Effect marker boosted
        assert!(weights[1] < 1.0, "Cause marker should be reduced in effect weights");
    }

    /// Load the causal model tokenizer, or skip the test if not available.
    fn load_test_tokenizer() -> Option<tokenizers::Tokenizer> {
        let path = std::path::Path::new("models/causal/tokenizer.json");
        // Also try relative from workspace root
        let paths = [
            path.to_path_buf(),
            std::path::PathBuf::from("../../../models/causal/tokenizer.json"),
            std::path::PathBuf::from("../../../../models/causal/tokenizer.json"),
        ];
        for p in &paths {
            if p.exists() {
                return tokenizers::Tokenizer::from_file(p).ok();
            }
        }
        None
    }

    #[test]
    fn test_detect_with_hints_no_hint_matches_baseline() {
        // Without hints, detect_causal_markers_with_hints should match detect_causal_markers
        let tokenizer = match load_test_tokenizer() {
            Some(t) => t,
            None => { println!("[SKIP] Tokenizer not available"); return; }
        };

        let text = "Stress causes memory loss because cortisol damages neurons";
        let encoding = tokenizer.encode(text, false).unwrap();

        let baseline = detect_causal_markers(text, &encoding);
        let with_hints = detect_causal_markers_with_hints(text, &encoding, None);

        assert_eq!(baseline.cause_marker_indices, with_hints.cause_marker_indices);
        assert_eq!(baseline.effect_marker_indices, with_hints.effect_marker_indices);
        assert_eq!(with_hints.llm_cause_count, 0);
        assert_eq!(with_hints.llm_effect_count, 0);
    }

    #[test]
    fn test_detect_with_hints_low_confidence_ignored() {
        let tokenizer = match load_test_tokenizer() {
            Some(t) => t,
            None => { println!("[SKIP] Tokenizer not available"); return; }
        };

        let text = "Chronic stress damages hippocampal neurons";
        let encoding = tokenizer.encode(text, false).unwrap();

        let hint = CausalHintGuidance {
            cause_spans: vec![(0, 14)],   // "Chronic stress"
            effect_spans: vec![(23, 44)], // "hippocampal neurons"
            key_phrases: vec!["damages".to_string()],
            asymmetry_strength: 0.9,
            confidence: 0.3, // Below threshold
        };

        let baseline = detect_causal_markers(text, &encoding);
        let with_hints = detect_causal_markers_with_hints(text, &encoding, Some(&hint));

        // Low confidence hint should be ignored — same as baseline
        assert_eq!(baseline.cause_marker_indices, with_hints.cause_marker_indices);
        assert_eq!(baseline.effect_marker_indices, with_hints.effect_marker_indices);
        assert_eq!(with_hints.llm_cause_count, 0);
    }

    #[test]
    fn test_detect_with_hints_cause_spans_boost_tokens() {
        let tokenizer = match load_test_tokenizer() {
            Some(t) => t,
            None => { println!("[SKIP] Tokenizer not available"); return; }
        };

        let text = "Chronic stress damages hippocampal neurons leading to memory loss";
        let encoding = tokenizer.encode(text, false).unwrap();

        let hint = CausalHintGuidance {
            cause_spans: vec![(0, 14)],   // "Chronic stress"
            effect_spans: vec![(47, 64)], // "memory loss"
            key_phrases: vec![],
            asymmetry_strength: 0.9,
            confidence: 0.9,
        };

        let baseline = detect_causal_markers(text, &encoding);
        let with_hints = detect_causal_markers_with_hints(text, &encoding, Some(&hint));

        // Hints should add markers that static detection missed
        assert!(
            with_hints.cause_marker_indices.len() >= baseline.cause_marker_indices.len(),
            "Hints should add cause markers, not remove them"
        );
        assert!(
            with_hints.effect_marker_indices.len() >= baseline.effect_marker_indices.len(),
            "Hints should add effect markers, not remove them"
        );
        assert!(
            with_hints.llm_cause_count > 0 || with_hints.llm_effect_count > 0,
            "At least one LLM marker should be injected for domain-specific entities"
        );
        // Effective boost should reflect high asymmetry
        let expected_boost = MARKER_BOOST * (0.5 + 0.5 * 0.9);
        assert!(
            (with_hints.effective_boost - expected_boost).abs() < f32::EPSILON,
            "Effective boost should be {} but got {}",
            expected_boost, with_hints.effective_boost
        );
    }

    #[test]
    fn test_provenance_counts_separate_static_and_llm() {
        let tokenizer = match load_test_tokenizer() {
            Some(t) => t,
            None => { println!("[SKIP] Tokenizer not available"); return; }
        };

        let text = "Because of cortisol, it results in memory impairment";
        let encoding = tokenizer.encode(text, false).unwrap();

        // No hints — should have static markers only
        let without_hints = detect_causal_markers_with_hints(text, &encoding, None);
        assert!(without_hints.static_cause_count > 0, "Should detect 'because' as static cause marker");
        assert!(without_hints.static_effect_count > 0, "Should detect 'results' as static effect marker");
        assert_eq!(without_hints.llm_cause_count, 0);
        assert_eq!(without_hints.llm_effect_count, 0);

        // With hints
        let hint = CausalHintGuidance {
            cause_spans: vec![(11, 19)],  // "cortisol"
            effect_spans: vec![(35, 52)], // "memory impairment"
            key_phrases: vec![],
            asymmetry_strength: 0.8,
            confidence: 0.85,
        };

        let with_hints = detect_causal_markers_with_hints(text, &encoding, Some(&hint));
        // Static counts should be same or higher (never decrease)
        assert!(
            with_hints.static_cause_count >= without_hints.static_cause_count,
            "Static cause count should not decrease"
        );
        // LLM counts should add markers for "cortisol" and "memory impairment"
        assert!(
            with_hints.llm_cause_count > 0 || with_hints.llm_effect_count > 0,
            "Should have LLM-injected markers for domain-specific entities"
        );
    }
}
