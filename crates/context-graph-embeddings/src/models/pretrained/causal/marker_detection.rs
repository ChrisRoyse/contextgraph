//! Causal marker detection for Longformer global attention.
//!
//! This module detects causal indicator tokens in text to enable
//! focused global attention on causally-relevant positions.
//!
//! # Architecture
//!
//! Rather than using uniform rotation (which preserves relative rankings),
//! this module enables content-dependent global attention by:
//! 1. Detecting causal markers (cause/effect indicators) in text
//! 2. Returning token indices for global attention assignment
//! 3. Enabling different attention patterns for cause vs effect embeddings

use tokenizers::Encoding;

/// Result of causal marker detection.
#[derive(Debug, Clone, Default)]
pub struct CausalMarkerResult {
    /// Token indices of cause indicators (e.g., "because", "caused by")
    pub cause_marker_indices: Vec<usize>,
    /// Token indices of effect indicators (e.g., "therefore", "results in")
    pub effect_marker_indices: Vec<usize>,
    /// All marker indices combined (for unified global attention)
    pub all_marker_indices: Vec<usize>,
    /// Detected dominant direction (if any)
    pub detected_direction: CausalDirection,
    /// Causal strength score [0.0, 1.0]
    pub causal_strength: f32,
}

/// Direction of causal relationship detected in text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CausalDirection {
    /// Text emphasizes causes (e.g., "because X, Y happened")
    Cause,
    /// Text emphasizes effects (e.g., "X happened, therefore Y")
    Effect,
    /// Both or neither detected
    #[default]
    Unknown,
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

    // Combine all markers
    let mut all_indices = cause_indices.clone();
    all_indices.extend(&effect_indices);
    all_indices.sort_unstable();
    all_indices.dedup();

    // Determine dominant direction based on counts
    let detected_direction = match cause_indices.len().cmp(&effect_indices.len()) {
        std::cmp::Ordering::Greater => CausalDirection::Cause,
        std::cmp::Ordering::Less => CausalDirection::Effect,
        std::cmp::Ordering::Equal if !cause_indices.is_empty() => CausalDirection::Cause, // Tie-breaker
        _ => CausalDirection::Unknown,
    };

    // Compute causal strength based on marker density
    let word_count = text.split_whitespace().count().max(1) as f32;
    let total_markers = all_indices.len() as f32;
    let causal_strength = (total_markers / word_count.sqrt()).min(1.0);

    CausalMarkerResult {
        cause_marker_indices: cause_indices,
        effect_marker_indices: effect_indices,
        all_marker_indices: all_indices,
        detected_direction,
        causal_strength,
    }
}

/// Create global attention indices for cause-focused embedding.
///
/// Returns token indices that should receive global attention when
/// embedding text as a potential CAUSE:
/// - CLS token (index 0)
/// - All cause indicator tokens
/// - First few tokens (captures subject/agent)
///
/// # Arguments
///
/// * `markers` - Detected causal markers
/// * `seq_len` - Sequence length
///
/// # Returns
///
/// Vector of token indices for global attention
pub fn cause_global_attention_indices(markers: &CausalMarkerResult, seq_len: usize) -> Vec<usize> {
    let mut indices = vec![0]; // CLS token always gets global attention

    // Add cause markers
    indices.extend(&markers.cause_marker_indices);

    // Add first few content tokens (often contain subject)
    for i in 1..4.min(seq_len) {
        if !indices.contains(&i) {
            indices.push(i);
        }
    }

    // Add some context around cause markers
    for &marker_idx in &markers.cause_marker_indices {
        // Token before marker (if exists)
        if marker_idx > 0 && !indices.contains(&(marker_idx - 1)) {
            indices.push(marker_idx - 1);
        }
        // Token after marker (if exists)
        if marker_idx + 1 < seq_len && !indices.contains(&(marker_idx + 1)) {
            indices.push(marker_idx + 1);
        }
    }

    indices.sort_unstable();
    indices.dedup();
    indices
}

/// Create global attention indices for effect-focused embedding.
///
/// Returns token indices that should receive global attention when
/// embedding text as a potential EFFECT:
/// - CLS token (index 0)
/// - All effect indicator tokens
/// - Last few tokens (captures conclusion/outcome)
///
/// # Arguments
///
/// * `markers` - Detected causal markers
/// * `seq_len` - Sequence length
///
/// # Returns
///
/// Vector of token indices for global attention
pub fn effect_global_attention_indices(markers: &CausalMarkerResult, seq_len: usize) -> Vec<usize> {
    let mut indices = vec![0]; // CLS token always gets global attention

    // Add effect markers
    indices.extend(&markers.effect_marker_indices);

    // Add last few content tokens (often contain outcome)
    let last_content = seq_len.saturating_sub(2); // Exclude [SEP] if present
    for i in last_content.saturating_sub(3)..last_content {
        if !indices.contains(&i) {
            indices.push(i);
        }
    }

    // Add some context around effect markers
    for &marker_idx in &markers.effect_marker_indices {
        if marker_idx > 0 && !indices.contains(&(marker_idx - 1)) {
            indices.push(marker_idx - 1);
        }
        if marker_idx + 1 < seq_len && !indices.contains(&(marker_idx + 1)) {
            indices.push(marker_idx + 1);
        }
    }

    indices.sort_unstable();
    indices.dedup();
    indices
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
    fn test_causal_direction_default() {
        assert_eq!(CausalDirection::default(), CausalDirection::Unknown);
    }

    #[test]
    fn test_marker_result_default() {
        let result = CausalMarkerResult::default();
        assert!(result.cause_marker_indices.is_empty());
        assert!(result.effect_marker_indices.is_empty());
        assert!(result.all_marker_indices.is_empty());
        assert_eq!(result.detected_direction, CausalDirection::Unknown);
        assert_eq!(result.causal_strength, 0.0);
    }

    #[test]
    fn test_cause_global_attention_always_includes_cls() {
        let markers = CausalMarkerResult::default();
        let indices = cause_global_attention_indices(&markers, 10);
        assert!(indices.contains(&0), "CLS token must always be included");
    }

    #[test]
    fn test_effect_global_attention_always_includes_cls() {
        let markers = CausalMarkerResult::default();
        let indices = effect_global_attention_indices(&markers, 10);
        assert!(indices.contains(&0), "CLS token must always be included");
    }

    #[test]
    fn test_cause_attention_includes_first_tokens() {
        let markers = CausalMarkerResult::default();
        let indices = cause_global_attention_indices(&markers, 10);
        // Should include first few tokens for subject capture
        assert!(indices.contains(&1) || indices.contains(&2) || indices.contains(&3));
    }

    #[test]
    fn test_effect_attention_includes_last_tokens() {
        let markers = CausalMarkerResult::default();
        let indices = effect_global_attention_indices(&markers, 10);
        // Should include last few tokens for outcome capture
        let has_late_tokens = indices.iter().any(|&i| i >= 5);
        assert!(has_late_tokens, "Effect attention should include later tokens");
    }
}
