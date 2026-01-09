//! Classification methods for JohariFingerprint.
//!
//! This module provides methods for classifying, setting, and querying
//! quadrant states for embedders.

use crate::types::JohariQuadrant;

use super::core::JohariFingerprint;
use super::NUM_EMBEDDERS;

impl JohariFingerprint {
    /// Classify based on entropy (delta S) and coherence (delta C) metrics.
    ///
    /// From constitution.yaml lines 188-194:
    /// - Open: entropy < 0.5 AND coherence > 0.5
    /// - Hidden: entropy < 0.5 AND coherence < 0.5
    /// - Blind: entropy > 0.5 AND coherence < 0.5
    /// - Unknown: entropy > 0.5 AND coherence > 0.5
    ///
    /// # Arguments
    /// * `entropy` - Entropy change value (delta S), typically in [0.0, 1.0]
    /// * `coherence` - Coherence change value (delta C), typically in [0.0, 1.0]
    ///
    /// # Returns
    /// The `JohariQuadrant` classification based on the UTL thresholds.
    ///
    /// # Boundary Behavior
    /// At exactly threshold (0.5), treats as:
    /// - entropy = 0.5 -> low entropy (< test uses >=)
    /// - coherence = 0.5 -> low coherence (> test uses >)
    #[inline]
    pub fn classify_quadrant(entropy: f32, coherence: f32) -> JohariQuadrant {
        let low_entropy = entropy < Self::ENTROPY_THRESHOLD;
        let high_coherence = coherence > Self::COHERENCE_THRESHOLD;

        match (low_entropy, high_coherence) {
            (true, true) => JohariQuadrant::Open,     // Low S, High C
            (true, false) => JohariQuadrant::Hidden,  // Low S, Low C
            (false, false) => JohariQuadrant::Blind,  // High S, Low C
            (false, true) => JohariQuadrant::Unknown, // High S, High C
        }
    }

    /// Get dominant (highest weight) quadrant for an embedder.
    ///
    /// # Arguments
    /// * `embedder_idx` - Index of embedder (0-12 for E1-E13)
    ///
    /// # Returns
    /// The `JohariQuadrant` with the highest weight for this embedder.
    /// If all weights are zero (unclassified), returns Unknown (the frontier state).
    /// In case of ties, returns the first (lowest index) tied quadrant.
    ///
    /// # Panics
    /// Panics if `embedder_idx >= NUM_EMBEDDERS` (13)
    #[inline]
    pub fn dominant_quadrant(&self, embedder_idx: usize) -> JohariQuadrant {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "embedder_idx {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );

        let weights = &self.quadrants[embedder_idx];

        // Check if all weights are zero (unclassified embedder)
        let sum: f32 = weights.iter().sum();
        if sum < f32::EPSILON {
            // Unclassified embedders default to Unknown (the frontier/exploration state)
            return JohariQuadrant::Unknown;
        }

        let mut max_idx = 0;
        let mut max_val = weights[0];

        for (idx, &val) in weights.iter().enumerate().skip(1) {
            if val > max_val {
                max_val = val;
                max_idx = idx;
            }
        }

        Self::idx_to_quadrant(max_idx)
    }

    /// Set quadrant weights for an embedder.
    ///
    /// Automatically normalizes so weights sum to 1.0.
    /// If all weights are 0, sets uniform distribution (0.25 each).
    ///
    /// # Arguments
    /// * `embedder_idx` - Index of embedder (0-12 for E1-E13)
    /// * `open` - Weight for Open quadrant
    /// * `hidden` - Weight for Hidden quadrant
    /// * `blind` - Weight for Blind quadrant
    /// * `unknown` - Weight for Unknown quadrant
    /// * `confidence` - Classification confidence [0.0, 1.0]
    ///
    /// # Panics
    /// - Panics if `embedder_idx >= NUM_EMBEDDERS` (13)
    /// - Panics if any weight is negative
    /// - Panics if confidence is not in [0.0, 1.0]
    pub fn set_quadrant(
        &mut self,
        embedder_idx: usize,
        open: f32,
        hidden: f32,
        blind: f32,
        unknown: f32,
        confidence: f32,
    ) {
        assert!(
            embedder_idx < NUM_EMBEDDERS,
            "embedder_idx {} out of bounds (max {})",
            embedder_idx,
            NUM_EMBEDDERS - 1
        );
        assert!(
            open >= 0.0,
            "open weight must be non-negative, got {}",
            open
        );
        assert!(
            hidden >= 0.0,
            "hidden weight must be non-negative, got {}",
            hidden
        );
        assert!(
            blind >= 0.0,
            "blind weight must be non-negative, got {}",
            blind
        );
        assert!(
            unknown >= 0.0,
            "unknown weight must be non-negative, got {}",
            unknown
        );
        assert!(
            (0.0..=1.0).contains(&confidence),
            "confidence must be in [0.0, 1.0], got {}",
            confidence
        );

        let sum = open + hidden + blind + unknown;

        if sum < f32::EPSILON {
            // All zero: use uniform distribution
            self.quadrants[embedder_idx] = [0.25, 0.25, 0.25, 0.25];
        } else {
            // Normalize to sum to 1.0
            self.quadrants[embedder_idx] = [open / sum, hidden / sum, blind / sum, unknown / sum];
        }

        self.confidence[embedder_idx] = confidence;
    }

    /// Find all embedder indices where the given quadrant is dominant.
    ///
    /// # Arguments
    /// * `quadrant` - The quadrant to search for
    ///
    /// # Returns
    /// Vector of embedder indices (0-12) where the given quadrant has the highest weight.
    pub fn find_by_quadrant(&self, quadrant: JohariQuadrant) -> Vec<usize> {
        (0..NUM_EMBEDDERS)
            .filter(|&idx| self.dominant_quadrant(idx) == quadrant)
            .collect()
    }

    /// Convert quadrant index to JohariQuadrant.
    #[inline]
    pub(crate) fn idx_to_quadrant(idx: usize) -> JohariQuadrant {
        match idx {
            0 => JohariQuadrant::Open,
            1 => JohariQuadrant::Hidden,
            2 => JohariQuadrant::Blind,
            _ => JohariQuadrant::Unknown,
        }
    }

    /// Convert JohariQuadrant to index.
    #[inline]
    pub(crate) fn quadrant_to_idx(quadrant: JohariQuadrant) -> usize {
        match quadrant {
            JohariQuadrant::Open => 0,
            JohariQuadrant::Hidden => 1,
            JohariQuadrant::Blind => 2,
            JohariQuadrant::Unknown => 3,
        }
    }
}
