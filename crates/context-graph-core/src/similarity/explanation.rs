//! Human-readable explanations for similarity results.
//!
//! This module provides the `SimilarityExplanation` struct for generating
//! understandable descriptions of why two fingerprints have a given similarity score.

use crate::types::fingerprint::NUM_EMBEDDERS;
use serde::{Deserialize, Serialize};

/// Human-readable explanation of a similarity result.
///
/// Provides detailed breakdown for debugging, logging, and user-facing
/// explanations of why two fingerprints have a given similarity score.
///
/// # Example
///
/// ```rust,ignore
/// let explanation = engine.explain(&result);
/// println!("{}", explanation.summary);
/// for detail in &explanation.space_details {
///     println!("  {}", detail);
/// }
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimilarityExplanation {
    /// One-line summary of the similarity result.
    ///
    /// Example: "High similarity (0.85) across 11/13 spaces, strong semantic alignment"
    pub summary: String,

    /// Per-space explanations (13 entries, None for inactive spaces).
    ///
    /// Each entry describes the contribution and significance of that space.
    pub space_details: [Option<SpaceDetail>; NUM_EMBEDDERS],

    /// Overall interpretation of the score.
    ///
    /// - "Very High": score >= 0.9
    /// - "High": score >= 0.7
    /// - "Moderate": score >= 0.5
    /// - "Low": score >= 0.3
    /// - "Very Low": score < 0.3
    pub score_interpretation: ScoreInterpretation,

    /// Key factors that influenced the score.
    ///
    /// List of the most significant positive and negative contributors.
    pub key_factors: Vec<String>,

    /// Confidence explanation.
    ///
    /// Describes why we are more or less confident in this score.
    pub confidence_explanation: String,

    /// Recommendations for improving similarity (if applicable).
    ///
    /// Only populated when score is low and improvement is possible.
    pub recommendations: Vec<String>,
}

/// Detail about a single embedding space's contribution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpaceDetail {
    /// Space index (0-12).
    pub space_idx: usize,

    /// Human-readable space name.
    ///
    /// E.g., "E1 Semantic", "E7 Code", "E12 ColBERT"
    pub space_name: String,

    /// Similarity score for this space.
    pub score: f32,

    /// Weight applied to this space.
    pub weight: f32,

    /// Weighted contribution (score * weight).
    pub contribution: f32,

    /// Interpretation of this space's result.
    pub interpretation: String,
}

/// Qualitative interpretation of a similarity score.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScoreInterpretation {
    /// Score >= 0.9 - Nearly identical
    VeryHigh,
    /// Score >= 0.7 - Strong match
    High,
    /// Score >= 0.5 - Moderate match
    Moderate,
    /// Score >= 0.3 - Weak match
    Low,
    /// Score < 0.3 - Poor match
    VeryLow,
}

impl ScoreInterpretation {
    /// Classify a score into an interpretation category.
    #[inline]
    pub fn from_score(score: f32) -> Self {
        if score >= 0.9 {
            Self::VeryHigh
        } else if score >= 0.7 {
            Self::High
        } else if score >= 0.5 {
            Self::Moderate
        } else if score >= 0.3 {
            Self::Low
        } else {
            Self::VeryLow
        }
    }

    /// Get human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::VeryHigh => "Very High",
            Self::High => "High",
            Self::Moderate => "Moderate",
            Self::Low => "Low",
            Self::VeryLow => "Very Low",
        }
    }

    /// Get description of what this interpretation means.
    pub fn description(&self) -> &'static str {
        match self {
            Self::VeryHigh => "Nearly identical content or highly related concepts",
            Self::High => "Strong semantic or structural similarity",
            Self::Moderate => "Some shared concepts or patterns",
            Self::Low => "Limited overlap, may share some context",
            Self::VeryLow => "Little to no meaningful similarity",
        }
    }
}

impl std::fmt::Display for ScoreInterpretation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Names for the 13 embedding spaces.
pub const SPACE_NAMES: [&str; NUM_EMBEDDERS] = [
    "E1 Semantic",
    "E2 Sentiment",
    "E3 Temporal",
    "E4 Spatial",
    "E5 Causal",
    "E6 Sparse",
    "E7 Code",
    "E8 Graph",
    "E9 Contextual",
    "E10 Multimodal",
    "E11 Entity",
    "E12 ColBERT",
    "E13 SPLADE",
];

impl SimilarityExplanation {
    /// Create a basic explanation from a score and active spaces.
    pub fn basic(score: f32, active_count: u32, confidence: f32) -> Self {
        let interpretation = ScoreInterpretation::from_score(score);
        let summary = format!(
            "{} similarity ({:.2}) across {}/{} spaces (confidence: {:.0}%)",
            interpretation.label(),
            score,
            active_count,
            NUM_EMBEDDERS,
            confidence * 100.0
        );

        let confidence_explanation = if confidence >= 0.8 {
            "High confidence: good space coverage with consistent scores".to_string()
        } else if confidence >= 0.5 {
            "Moderate confidence: reasonable space coverage".to_string()
        } else {
            "Low confidence: limited space coverage or high variance".to_string()
        };

        Self {
            summary,
            space_details: [const { None }; NUM_EMBEDDERS],
            score_interpretation: interpretation,
            key_factors: Vec::new(),
            confidence_explanation,
            recommendations: Vec::new(),
        }
    }

    /// Create a detailed explanation with per-space breakdown.
    pub fn detailed(
        score: f32,
        confidence: f32,
        space_scores: &[Option<f32>; NUM_EMBEDDERS],
        space_weights: &[f32; NUM_EMBEDDERS],
    ) -> Self {
        let interpretation = ScoreInterpretation::from_score(score);
        let mut active_count = 0u32;
        let mut space_details: [Option<SpaceDetail>; NUM_EMBEDDERS] =
            [const { None }; NUM_EMBEDDERS];
        let mut key_factors = Vec::new();
        let mut high_contributors = Vec::new();
        let mut low_contributors = Vec::new();

        for (i, (score_opt, &weight)) in space_scores.iter().zip(space_weights.iter()).enumerate() {
            if let Some(s) = score_opt {
                active_count += 1;
                let contribution = s * weight;
                let space_interp = ScoreInterpretation::from_score(*s);

                space_details[i] = Some(SpaceDetail {
                    space_idx: i,
                    space_name: SPACE_NAMES[i].to_string(),
                    score: *s,
                    weight,
                    contribution,
                    interpretation: format!(
                        "{} ({:.2} × {:.3} = {:.3})",
                        space_interp.label(),
                        s,
                        weight,
                        contribution
                    ),
                });

                // Track high and low contributors for key factors
                if *s >= 0.7 && weight >= 0.05 {
                    high_contributors.push((i, *s, contribution));
                }
                if *s < 0.3 && weight >= 0.05 {
                    low_contributors.push((i, *s, contribution));
                }
            }
        }

        // Build key factors
        high_contributors
            .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        low_contributors.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

        for (idx, s, _) in high_contributors.iter().take(3) {
            key_factors.push(format!("+ Strong {} match ({:.2})", SPACE_NAMES[*idx], s));
        }

        for (idx, s, _) in low_contributors.iter().take(2) {
            key_factors.push(format!("- Weak {} match ({:.2})", SPACE_NAMES[*idx], s));
        }

        let summary = format!(
            "{} similarity ({:.2}) across {}/{} spaces (confidence: {:.0}%)",
            interpretation.label(),
            score,
            active_count,
            NUM_EMBEDDERS,
            confidence * 100.0
        );

        let confidence_explanation = Self::explain_confidence(confidence, active_count);

        // Generate recommendations for low scores
        let recommendations = if score < 0.5 {
            Self::generate_recommendations(&low_contributors)
        } else {
            Vec::new()
        };

        Self {
            summary,
            space_details,
            score_interpretation: interpretation,
            key_factors,
            confidence_explanation,
            recommendations,
        }
    }

    fn explain_confidence(confidence: f32, active_count: u32) -> String {
        if confidence >= 0.8 {
            format!(
                "High confidence: {} active spaces with consistent scores",
                active_count
            )
        } else if confidence >= 0.5 {
            if active_count < 8 {
                format!(
                    "Moderate confidence: only {} spaces active (some embeddings missing)",
                    active_count
                )
            } else {
                "Moderate confidence: some variance in per-space scores".to_string()
            }
        } else if active_count < 5 {
            format!(
                "Low confidence: only {} spaces active (insufficient data)",
                active_count
            )
        } else {
            "Low confidence: high variance in per-space scores".to_string()
        }
    }

    fn generate_recommendations(low_contributors: &[(usize, f32, f32)]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for (idx, _, _) in low_contributors.iter().take(2) {
            let space_name = SPACE_NAMES[*idx];
            let rec = match *idx {
                0 => "Consider enriching semantic content or using more descriptive text",
                4 => "Add causal relationships or temporal ordering to improve causal similarity",
                6 => "Code similarity is low - check for structural or syntactic differences",
                7 => "Graph structure differs - review entity relationships",
                10 => "Entity overlap is low - ensure key entities are present in both",
                _ => "Review content alignment for this embedding space",
            };
            recommendations.push(format!("{}: {}", space_name, rec));
        }

        recommendations
    }

    /// Get a list of active space indices.
    pub fn active_spaces(&self) -> Vec<usize> {
        self.space_details
            .iter()
            .enumerate()
            .filter_map(|(i, detail)| detail.as_ref().map(|_| i))
            .collect()
    }

    /// Get the dominant contributing space (highest weighted contribution).
    pub fn dominant_space(&self) -> Option<&SpaceDetail> {
        self.space_details
            .iter()
            .filter_map(|d| d.as_ref())
            .max_by(|a, b| {
                a.contribution
                    .partial_cmp(&b.contribution)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }
}

impl Default for SimilarityExplanation {
    fn default() -> Self {
        Self::basic(0.0, 0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_interpretation_thresholds() {
        assert_eq!(
            ScoreInterpretation::from_score(0.95),
            ScoreInterpretation::VeryHigh
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.90),
            ScoreInterpretation::VeryHigh
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.85),
            ScoreInterpretation::High
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.70),
            ScoreInterpretation::High
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.55),
            ScoreInterpretation::Moderate
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.50),
            ScoreInterpretation::Moderate
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.35),
            ScoreInterpretation::Low
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.30),
            ScoreInterpretation::Low
        );
        assert_eq!(
            ScoreInterpretation::from_score(0.20),
            ScoreInterpretation::VeryLow
        );

        println!("[PASS] Score interpretation thresholds are correct");
    }

    #[test]
    fn test_basic_explanation() {
        let explanation = SimilarityExplanation::basic(0.75, 10, 0.85);

        assert!(explanation.summary.contains("High"));
        assert!(explanation.summary.contains("0.75"));
        assert!(explanation.summary.contains("10/13"));
        assert_eq!(explanation.score_interpretation, ScoreInterpretation::High);
        assert!(explanation
            .confidence_explanation
            .contains("High confidence"));

        println!("[PASS] Basic explanation: {}", explanation.summary);
    }

    #[test]
    fn test_detailed_explanation() {
        let mut space_scores: [Option<f32>; NUM_EMBEDDERS] = [None; NUM_EMBEDDERS];
        space_scores[0] = Some(0.9); // E1 Semantic - high
        space_scores[4] = Some(0.8); // E5 Causal - high
        space_scores[6] = Some(0.2); // E7 Code - low
        space_scores[10] = Some(0.7); // E11 Entity - moderate

        let weights = [
            0.15, 0.08, 0.08, 0.08, 0.12, 0.05, 0.10, 0.08, 0.08, 0.05, 0.10, 0.02, 0.01,
        ];

        let explanation = SimilarityExplanation::detailed(0.72, 0.65, &space_scores, &weights);

        assert_eq!(explanation.score_interpretation, ScoreInterpretation::High);
        assert!(explanation.space_details[0].is_some());
        assert!(explanation.space_details[1].is_none());
        assert!(!explanation.key_factors.is_empty());

        println!("[PASS] Detailed explanation: {}", explanation.summary);
        for factor in &explanation.key_factors {
            println!("  Factor: {}", factor);
        }
    }

    #[test]
    fn test_space_names_count() {
        assert_eq!(SPACE_NAMES.len(), NUM_EMBEDDERS);
        assert_eq!(SPACE_NAMES[0], "E1 Semantic");
        assert_eq!(SPACE_NAMES[12], "E13 SPLADE");

        println!("[PASS] All 13 space names defined correctly");
    }

    #[test]
    fn test_active_spaces() {
        let mut space_scores: [Option<f32>; NUM_EMBEDDERS] = [None; NUM_EMBEDDERS];
        space_scores[0] = Some(0.9);
        space_scores[2] = Some(0.5);
        space_scores[5] = Some(0.6);

        let weights = [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS];
        let explanation = SimilarityExplanation::detailed(0.6, 0.5, &space_scores, &weights);

        let active = explanation.active_spaces();
        assert_eq!(active.len(), 3);
        assert!(active.contains(&0));
        assert!(active.contains(&2));
        assert!(active.contains(&5));

        println!("[PASS] Active spaces correctly identified: {:?}", active);
    }

    #[test]
    fn test_dominant_space() {
        let mut space_scores: [Option<f32>; NUM_EMBEDDERS] = [None; NUM_EMBEDDERS];
        space_scores[0] = Some(0.5); // E1 - medium score but high weight
        space_scores[6] = Some(0.9); // E7 - high score but lower weight

        let mut weights = [0.05; NUM_EMBEDDERS];
        weights[0] = 0.3; // E1 has much higher weight

        let explanation = SimilarityExplanation::detailed(0.6, 0.5, &space_scores, &weights);

        let dominant = explanation
            .dominant_space()
            .expect("Should have dominant space");
        assert_eq!(dominant.space_idx, 0); // E1 should dominate due to weight

        println!(
            "[PASS] Dominant space: {} with contribution {:.3}",
            dominant.space_name, dominant.contribution
        );
    }

    #[test]
    fn test_low_score_recommendations() {
        let mut space_scores: [Option<f32>; NUM_EMBEDDERS] = [None; NUM_EMBEDDERS];
        space_scores[0] = Some(0.2); // E1 low
        space_scores[6] = Some(0.1); // E7 Code very low

        let weights = [
            0.15, 0.08, 0.08, 0.08, 0.12, 0.05, 0.10, 0.08, 0.08, 0.05, 0.10, 0.02, 0.01,
        ];

        let explanation = SimilarityExplanation::detailed(0.15, 0.3, &space_scores, &weights);

        assert!(!explanation.recommendations.is_empty());
        println!("[PASS] Recommendations for low score:");
        for rec in &explanation.recommendations {
            println!("  - {}", rec);
        }
    }
}
