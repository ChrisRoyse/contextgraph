//! JohariFingerprint: Per-embedder awareness classification.
//!
//! **STATUS: STUB - AWAITING TASK-F003 COMPLETION**
//!
//! This module provides a minimal placeholder for JohariFingerprint
//! to allow TeleologicalFingerprint to compile. The full implementation
//! is defined in TASK-F003.
//!
//! From constitution.yaml, the Johari Window maps to ΔS × ΔC:
//! - Open: Low entropy (ΔS), High coherence (ΔC) - Known to self AND others
//! - Hidden: Medium entropy, High coherence - Known to self, NOT others
//! - Blind: High entropy, Low coherence - NOT known to self, Known to others
//! - Unknown: High entropy, Unknown coherence - NOT known to self OR others

use serde::{Deserialize, Serialize};

use crate::types::JohariQuadrant;

use super::purpose::NUM_EMBEDDERS;

/// Per-embedder Johari awareness classification.
///
/// Each of the 13 embedders has its own Johari quadrant, indicating
/// how "aware" the system is of that semantic dimension.
///
/// **NOTE**: This is a stub implementation. Full implementation in TASK-F003.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JohariFingerprint {
    /// Johari quadrant for each of the 13 embedders.
    /// Index mapping matches PurposeVector (E1-E13).
    pub quadrants: [JohariQuadrant; NUM_EMBEDDERS],

    /// The dominant (most common) quadrant across all embedders.
    pub dominant_quadrant: JohariQuadrant,

    /// Fraction of embedders in the Open quadrant [0.0, 1.0].
    pub openness: f32,
}

impl JohariFingerprint {
    /// Create a new JohariFingerprint from quadrant classifications.
    ///
    /// **NOTE**: Stub implementation - TASK-F003 will add full logic.
    pub fn new(quadrants: [JohariQuadrant; NUM_EMBEDDERS]) -> Self {
        let dominant_quadrant = Self::compute_dominant(&quadrants);
        let openness = Self::compute_openness(&quadrants);

        Self {
            quadrants,
            dominant_quadrant,
            openness,
        }
    }

    /// Create a stub with all embedders in Unknown quadrant.
    ///
    /// Used when TASK-F003 is not complete but F002 needs to compile.
    pub fn stub() -> Self {
        Self::new([JohariQuadrant::Unknown; NUM_EMBEDDERS])
    }

    /// Check if overall awareness is healthy (majority Open/Hidden).
    pub fn is_aware(&self) -> bool {
        self.openness >= 0.5
    }

    fn compute_dominant(quadrants: &[JohariQuadrant; NUM_EMBEDDERS]) -> JohariQuadrant {
        let mut counts = [0u8; 4]; // Open, Hidden, Blind, Unknown

        for q in quadrants {
            match q {
                JohariQuadrant::Open => counts[0] += 1,
                JohariQuadrant::Hidden => counts[1] += 1,
                JohariQuadrant::Blind => counts[2] += 1,
                JohariQuadrant::Unknown => counts[3] += 1,
            }
        }

        let max_idx = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &count)| count)
            .map(|(idx, _)| idx)
            .unwrap_or(3);

        match max_idx {
            0 => JohariQuadrant::Open,
            1 => JohariQuadrant::Hidden,
            2 => JohariQuadrant::Blind,
            _ => JohariQuadrant::Unknown,
        }
    }

    fn compute_openness(quadrants: &[JohariQuadrant; NUM_EMBEDDERS]) -> f32 {
        let open_count = quadrants
            .iter()
            .filter(|&&q| q == JohariQuadrant::Open)
            .count();
        open_count as f32 / NUM_EMBEDDERS as f32
    }
}

impl Default for JohariFingerprint {
    /// Default to all Unknown (stub behavior).
    fn default() -> Self {
        Self::stub()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_johari_fingerprint_stub() {
        let jf = JohariFingerprint::stub();

        assert_eq!(jf.quadrants, [JohariQuadrant::Unknown; NUM_EMBEDDERS]);
        assert_eq!(jf.dominant_quadrant, JohariQuadrant::Unknown);
        assert_eq!(jf.openness, 0.0);

        println!("[PASS] JohariFingerprint::stub creates all Unknown");
    }

    #[test]
    fn test_johari_fingerprint_new() {
        let mut quadrants = [JohariQuadrant::Open; NUM_EMBEDDERS];
        quadrants[0] = JohariQuadrant::Hidden;
        quadrants[1] = JohariQuadrant::Blind;

        let jf = JohariFingerprint::new(quadrants);

        // With 13 embedders, 2 non-Open means 11/13 Open
        let expected_openness = (NUM_EMBEDDERS - 2) as f32 / NUM_EMBEDDERS as f32;
        assert_eq!(jf.dominant_quadrant, JohariQuadrant::Open); // 11/13 Open
        assert!((jf.openness - expected_openness).abs() < f32::EPSILON);

        println!("[PASS] JohariFingerprint::new computes correct dominant and openness");
    }

    #[test]
    fn test_johari_fingerprint_is_aware() {
        // Majority Open = aware
        let aware = JohariFingerprint::new([JohariQuadrant::Open; NUM_EMBEDDERS]);
        assert!(aware.is_aware());

        // Majority Unknown = not aware
        let unaware = JohariFingerprint::stub();
        assert!(!unaware.is_aware());

        println!("[PASS] JohariFingerprint::is_aware returns correct value");
    }

    #[test]
    fn test_johari_fingerprint_default() {
        let jf = JohariFingerprint::default();
        assert_eq!(jf.quadrants, [JohariQuadrant::Unknown; NUM_EMBEDDERS]);

        println!("[PASS] JohariFingerprint::default is stub");
    }
}
