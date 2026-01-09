//! Alignment-related methods for TeleologicalFingerprint.
//!
//! This module contains methods for computing and checking alignment status.

use crate::types::fingerprint::purpose::AlignmentThreshold;

use super::types::TeleologicalFingerprint;

impl TeleologicalFingerprint {
    /// Compute the alignment delta from the previous snapshot.
    ///
    /// Returns 0.0 if there is only one snapshot (no previous to compare).
    ///
    /// # Returns
    /// `current_alignment - previous_alignment`
    /// Negative values indicate alignment is degrading.
    pub fn compute_alignment_delta(&self) -> f32 {
        if self.purpose_evolution.len() < 2 {
            return 0.0;
        }

        let current = self.theta_to_north_star;
        let previous =
            self.purpose_evolution[self.purpose_evolution.len() - 2].aggregate_alignment();

        current - previous
    }

    /// Check for misalignment warning.
    ///
    /// From constitution.yaml: delta_A < -0.15 predicts failure 72 hours ahead.
    ///
    /// # Returns
    /// `Some(delta_a)` if misalignment detected, `None` otherwise.
    pub fn check_misalignment_warning(&self) -> Option<f32> {
        let delta = self.compute_alignment_delta();
        if delta < Self::MISALIGNMENT_THRESHOLD {
            Some(delta)
        } else {
            None
        }
    }

    /// Get the current alignment status.
    pub fn alignment_status(&self) -> AlignmentThreshold {
        AlignmentThreshold::classify(self.theta_to_north_star)
    }

    /// Check if this fingerprint has concerning alignment trends.
    ///
    /// Returns true if:
    /// - Current alignment is in Warning or Critical threshold
    /// - OR alignment delta indicates degradation (< -0.15)
    pub fn is_concerning(&self) -> bool {
        self.alignment_status().is_misaligned() || self.check_misalignment_warning().is_some()
    }

    /// Get a summary of alignment history.
    ///
    /// Returns (min, max, average) alignment across all snapshots.
    pub fn alignment_history_stats(&self) -> (f32, f32, f32) {
        if self.purpose_evolution.is_empty() {
            return (0.0, 0.0, 0.0);
        }

        let mut min = f32::MAX;
        let mut max = f32::MIN;
        let mut sum = 0.0f32;

        for snapshot in &self.purpose_evolution {
            let alignment = snapshot.aggregate_alignment();
            min = min.min(alignment);
            max = max.max(alignment);
            sum += alignment;
        }

        let avg = sum / self.purpose_evolution.len() as f32;
        (min, max, avg)
    }
}
