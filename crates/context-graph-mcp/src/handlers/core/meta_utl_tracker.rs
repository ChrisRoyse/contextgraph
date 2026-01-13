//! Meta-UTL Tracker for learning about learning.
//!
//! TASK-S005: Tracks per-embedder accuracy, pending predictions, and optimized weights.
//! TASK-METAUTL-P0-001: Extended with consecutive low tracking and weight clamping.

use std::collections::HashMap;
use std::time::Instant;

use tracing::debug;
use uuid::Uuid;

use context_graph_core::johari::NUM_EMBEDDERS;

use super::types::{Domain, DomainAccuracyTracker, SelfCorrectionConfig, StoredPrediction};

/// Meta-UTL Tracker for learning about learning
///
/// TASK-S005: Tracks per-embedder accuracy, pending predictions, and optimized weights.
/// TASK-METAUTL-P0-001: Extended with consecutive low tracking and weight clamping.
/// Uses rolling window for accuracy tracking to maintain recency bias.
#[derive(Debug)]
pub struct MetaUtlTracker {
    /// Pending predictions awaiting validation
    pub pending_predictions: HashMap<Uuid, StoredPrediction>,
    /// Per-embedder accuracy rolling window (100 samples per embedder)
    pub embedder_accuracy: [[f32; 100]; NUM_EMBEDDERS],
    /// Current index in each embedder's rolling window
    pub accuracy_indices: [usize; NUM_EMBEDDERS],
    /// Number of samples in each embedder's rolling window
    pub accuracy_counts: [usize; NUM_EMBEDDERS],
    /// Current optimized weights (sum to 1.0, clamped to [0.05, 0.9] per constitution)
    pub current_weights: [f32; NUM_EMBEDDERS],
    /// Total predictions made
    pub prediction_count: usize,
    /// Total validations completed
    pub validation_count: usize,
    /// Last weight update timestamp
    pub last_weight_update: Option<Instant>,
    /// TASK-METAUTL-P0-001: Consecutive cycles with accuracy < 0.7
    pub consecutive_low_count: usize,
    /// TASK-METAUTL-P0-001: Whether Bayesian escalation has been triggered
    pub escalation_triggered: bool,
    /// TASK-METAUTL-P0-001: Self-correction configuration
    pub config: SelfCorrectionConfig,
    /// TASK-METAUTL-P0-001: Tracks which embedders have been updated in current cycle
    cycle_embedder_updated: [bool; NUM_EMBEDDERS],
    /// TASK-METAUTL-P0-001: Number of complete accuracy recording cycles
    cycle_count: usize,
    /// TASK-METAUTL-P1-001: Per-domain accuracy tracking for lambda recalibration
    #[allow(dead_code)]
    pub domain_accuracy: HashMap<Domain, DomainAccuracyTracker>,
}

impl Default for MetaUtlTracker {
    fn default() -> Self {
        // Initialize with uniform weights (1/13 each)
        let initial_weight = 1.0 / NUM_EMBEDDERS as f32;
        Self {
            pending_predictions: HashMap::new(),
            embedder_accuracy: [[0.0; 100]; NUM_EMBEDDERS],
            accuracy_indices: [0; NUM_EMBEDDERS],
            accuracy_counts: [0; NUM_EMBEDDERS],
            current_weights: [initial_weight; NUM_EMBEDDERS],
            prediction_count: 0,
            validation_count: 0,
            last_weight_update: None,
            // TASK-METAUTL-P0-001: Initialize consecutive tracking
            consecutive_low_count: 0,
            escalation_triggered: false,
            config: SelfCorrectionConfig::default(),
            cycle_embedder_updated: [false; NUM_EMBEDDERS],
            cycle_count: 0,
            // TASK-METAUTL-P1-001: Per-domain accuracy tracking
            domain_accuracy: HashMap::new(),
        }
    }
}

impl MetaUtlTracker {
    /// Weight precision tolerance for sum normalization
    #[allow(dead_code)]
    const WEIGHT_PRECISION: f32 = 1e-6;

    /// Threshold for detecting accuracy trend changes
    #[allow(dead_code)]
    const TREND_THRESHOLD: f32 = 0.02;

    /// Create a new MetaUtlTracker
    pub fn new() -> Self {
        Self::default()
    }

    /// Store a prediction for later validation
    pub fn store_prediction(&mut self, prediction_id: Uuid, prediction: StoredPrediction) {
        self.pending_predictions.insert(prediction_id, prediction);
        self.prediction_count += 1;
    }

    /// Get a pending prediction by ID
    #[allow(dead_code)]
    pub fn get_prediction(&self, prediction_id: &Uuid) -> Option<&StoredPrediction> {
        self.pending_predictions.get(prediction_id)
    }

    /// Remove and return a prediction (for validation)
    pub fn remove_prediction(&mut self, prediction_id: &Uuid) -> Option<StoredPrediction> {
        self.pending_predictions.remove(prediction_id)
    }

    /// Record accuracy for an embedder
    ///
    /// TASK-METAUTL-P0-001: Also tracks consecutive low accuracy cycles.
    /// A "cycle" is considered complete when all 13 embedders have been updated.
    /// Low accuracy is defined as < 0.7 (constitution).
    pub fn record_accuracy(&mut self, embedder_index: usize, accuracy: f32) {
        if embedder_index >= NUM_EMBEDDERS {
            debug!(
                embedder_index = embedder_index,
                "record_accuracy: invalid embedder index, ignoring"
            );
            return;
        }

        // Clamp accuracy to [0.0, 1.0]
        let clamped_accuracy = accuracy.clamp(0.0, 1.0);

        let idx = self.accuracy_indices[embedder_index];
        self.embedder_accuracy[embedder_index][idx] = clamped_accuracy;
        self.accuracy_indices[embedder_index] = (idx + 1) % 100;
        if self.accuracy_counts[embedder_index] < 100 {
            self.accuracy_counts[embedder_index] += 1;
        }

        // TASK-METAUTL-P0-001: Track cycle completion
        self.cycle_embedder_updated[embedder_index] = true;

        // Check if a complete cycle has occurred (all embedders updated)
        if self.cycle_embedder_updated.iter().all(|&updated| updated) {
            self.cycle_count += 1;
            // Reset cycle tracking for next cycle
            self.cycle_embedder_updated = [false; NUM_EMBEDDERS];
            // Check consecutive low accuracy at end of cycle
            self.check_consecutive_low_accuracy();
        }
    }

    /// Check if overall accuracy is low and track consecutive count.
    ///
    /// TASK-METAUTL-P0-001: Called at the END of each complete cycle (when all
    /// 13 embedders have been recorded). This ensures we count cycles, not
    /// individual record_accuracy calls.
    fn check_consecutive_low_accuracy(&mut self) {
        // Calculate overall accuracy across all embedders
        let mut total_accuracy = 0.0f32;
        let mut embedder_count = 0usize;

        for i in 0..NUM_EMBEDDERS {
            if let Some(acc) = self.get_embedder_accuracy(i) {
                total_accuracy += acc;
                embedder_count += 1;
            }
        }

        // Only check if we have data from all embedders
        if embedder_count < NUM_EMBEDDERS {
            return;
        }

        let overall_accuracy = total_accuracy / embedder_count as f32;
        let threshold = self.config.low_accuracy_threshold; // 0.7

        if overall_accuracy < threshold {
            self.consecutive_low_count += 1;
            debug!(
                overall_accuracy = overall_accuracy,
                threshold = threshold,
                consecutive_low_count = self.consecutive_low_count,
                cycle_count = self.cycle_count,
                "Meta-UTL: low accuracy cycle recorded"
            );

            // Check if escalation should be triggered
            if self.consecutive_low_count >= self.config.max_consecutive_failures
                && !self.escalation_triggered
            {
                self.escalation_triggered = true;
                tracing::warn!(
                    consecutive_low = self.consecutive_low_count,
                    threshold = self.config.max_consecutive_failures,
                    "TASK-METAUTL-P0-001: Bayesian escalation triggered"
                );
            }
        } else {
            // Reset consecutive count on recovery
            if self.consecutive_low_count > 0 {
                debug!(
                    previous_count = self.consecutive_low_count,
                    overall_accuracy = overall_accuracy,
                    "Meta-UTL: accuracy recovered, resetting consecutive low count"
                );
            }
            self.consecutive_low_count = 0;
            // Note: We don't reset escalation_triggered here - that requires explicit reset
        }
    }

    /// Get average accuracy for an embedder
    pub fn get_embedder_accuracy(&self, embedder_index: usize) -> Option<f32> {
        if embedder_index >= NUM_EMBEDDERS || self.accuracy_counts[embedder_index] == 0 {
            return None;
        }
        let count = self.accuracy_counts[embedder_index];
        let sum: f32 = self.embedder_accuracy[embedder_index][..count].iter().sum();
        Some(sum / count as f32)
    }

    /// Get accuracy trend for an embedder (recent vs older samples)
    pub fn get_accuracy_trend(&self, embedder_index: usize) -> Option<&'static str> {
        if embedder_index >= NUM_EMBEDDERS || self.accuracy_counts[embedder_index] < 10 {
            return None;
        }
        let count = self.accuracy_counts[embedder_index];
        let recent_start = count.saturating_sub(10);
        let recent_sum: f32 = self.embedder_accuracy[embedder_index][recent_start..count]
            .iter()
            .sum();
        let recent_avg = recent_sum / 10.0;

        let older_end = if count >= 20 {
            count - 10
        } else {
            count - (count / 2)
        };
        let older_start = older_end.saturating_sub(10);
        let older_sum: f32 = self.embedder_accuracy[embedder_index][older_start..older_end]
            .iter()
            .sum();
        let older_count = older_end - older_start;
        if older_count == 0 {
            return Some("stable");
        }
        let older_avg = older_sum / older_count as f32;

        if recent_avg > older_avg + Self::TREND_THRESHOLD {
            Some("improving")
        } else if recent_avg < older_avg - Self::TREND_THRESHOLD {
            Some("declining")
        } else {
            Some("stable")
        }
    }

    /// Redistribute surplus from over-max weights to below-max weights.
    /// Returns the total surplus that was redistributed.
    fn redistribute_excess_weight(&mut self, max_weight: f32) -> f32 {
        let mut total_surplus = 0.0f32;
        let mut capped_count = 0usize;

        // Find weights above max
        for &weight in self.current_weights.iter() {
            if weight > max_weight {
                total_surplus += weight - max_weight;
                capped_count += 1;
            }
        }

        if total_surplus < Self::WEIGHT_PRECISION {
            return 0.0; // No surplus to redistribute
        }

        // Count weights below max that can receive redistribution
        let below_max_count = NUM_EMBEDDERS - capped_count;
        if below_max_count == 0 {
            // All weights at or above max - just cap them all
            for weight in self.current_weights.iter_mut() {
                if *weight > max_weight {
                    debug!(
                        original_weight = *weight,
                        clamped_weight = max_weight,
                        "TASK-METAUTL-P0-001: Lambda weight clamped to maximum"
                    );
                    *weight = max_weight;
                }
            }
            return total_surplus;
        }

        // Calculate how much each below-max weight should receive
        let redistribution = total_surplus / below_max_count as f32;

        // Apply capping and redistribution
        for weight in self.current_weights.iter_mut() {
            if *weight > max_weight {
                debug!(
                    original_weight = *weight,
                    clamped_weight = max_weight,
                    "TASK-METAUTL-P0-001: Lambda weight clamped to maximum"
                );
                *weight = max_weight;
            } else {
                *weight += redistribution;
            }
        }

        total_surplus
    }

    /// Update weights based on accuracy (called every 100 validations)
    ///
    /// TASK-METAUTL-P0-001: REQ-METAUTL-006/007 compliance.
    ///
    /// Priority of constraints:
    /// 1. Sum = 1.0 (REQ-METAUTL-006, HARD)
    /// 2. Max weight ≤ 0.9 (HARD - prevents single embedder dominance)
    /// 3. Min weight ≥ 0.05 (SOFT - best effort, may be violated in extreme cases)
    ///
    /// Algorithm:
    /// 1. Normalize weights based on accuracy (sum=1.0)
    /// 2. Cap any weight above max, redistribute surplus proportionally
    /// 3. Final normalization to ensure exact sum=1.0
    pub fn update_weights(&mut self) {
        // Calculate average accuracy per embedder
        let mut accuracies = [0.0f32; NUM_EMBEDDERS];
        let mut total_accuracy = 0.0f32;

        for (i, acc) in accuracies.iter_mut().enumerate() {
            *acc = self
                .get_embedder_accuracy(i)
                .unwrap_or(1.0 / NUM_EMBEDDERS as f32);
            total_accuracy += *acc;
        }

        // Normalize to get initial weights (sum = 1.0)
        if total_accuracy > 0.0 {
            for (weight, &acc) in self.current_weights.iter_mut().zip(accuracies.iter()) {
                *weight = acc / total_accuracy;
            }
        }

        let max_weight = self.config.max_weight; // 0.9
        let mut clamping_occurred = false;

        // STEP 1: Cap weights above max and redistribute surplus
        // This is the HARD constraint for max weight
        // Loop until no more surplus needs redistribution
        loop {
            let surplus = self.redistribute_excess_weight(max_weight);
            if surplus > 0.0 {
                clamping_occurred = true;
            }
            if surplus < Self::WEIGHT_PRECISION {
                break;
            }
        }

        // STEP 2: Final normalization to ensure exact sum=1.0
        let weight_sum: f32 = self.current_weights.iter().sum();
        if (weight_sum - 1.0).abs() > Self::WEIGHT_PRECISION {
            let scale = 1.0 / weight_sum;
            for weight in self.current_weights.iter_mut() {
                *weight *= scale;
            }
        }

        // Note: min_weight is a SOFT constraint. In extreme distributions
        // where one embedder dominates, other weights may be below min_weight
        // to maintain sum=1.0. This is mathematically necessary.
        // See EC-001 test for documentation.

        self.last_weight_update = Some(Instant::now());

        tracing::info!(
            validation_count = self.validation_count,
            weights_sum = self.current_weights.iter().sum::<f32>(),
            clamping_occurred = clamping_occurred,
            "Meta-UTL weights updated"
        );
    }

    /// Increment validation count and check if weights need update
    pub fn record_validation(&mut self) {
        self.validation_count += 1;
        if self.validation_count.is_multiple_of(100) {
            self.update_weights();
        }
    }

    /// Check if Bayesian escalation is needed.
    ///
    /// TASK-METAUTL-P0-001: Returns true when accuracy has been below 0.7
    /// for 10 or more consecutive cycles.
    #[allow(dead_code)] // API reserved for TASK-METAUTL-P0-005 integration
    pub fn needs_escalation(&self) -> bool {
        self.escalation_triggered
    }

    /// Get the current consecutive low accuracy count.
    ///
    /// TASK-METAUTL-P0-001: Returns the number of consecutive cycles
    /// with overall accuracy below 0.7.
    #[allow(dead_code)] // API reserved for TASK-METAUTL-P0-005 integration
    pub fn consecutive_low_count(&self) -> usize {
        self.consecutive_low_count
    }

    /// Reset the consecutive low accuracy counter and escalation flag.
    ///
    /// TASK-METAUTL-P0-001: Call this after taking corrective action
    /// (e.g., after Bayesian optimization completes).
    #[allow(dead_code)] // API reserved for TASK-METAUTL-P0-005 integration
    pub fn reset_consecutive_low(&mut self) {
        if self.consecutive_low_count > 0 || self.escalation_triggered {
            debug!(
                previous_count = self.consecutive_low_count,
                was_escalated = self.escalation_triggered,
                "TASK-METAUTL-P0-001: Resetting consecutive low tracking"
            );
        }
        self.consecutive_low_count = 0;
        self.escalation_triggered = false;
    }

    /// Get the self-correction configuration.
    ///
    /// TASK-METAUTL-P0-001: Provides access to constitution-defined parameters.
    #[allow(dead_code)] // API reserved for TASK-METAUTL-P0-005 integration
    pub fn config(&self) -> &SelfCorrectionConfig {
        &self.config
    }

    // ========== TASK-METAUTL-P1-001: Per-Domain Accuracy Tracking ==========

    /// Record accuracy for a specific domain.
    /// TASK-METAUTL-P1-001: Enables domain-specific lambda optimization.
    #[allow(dead_code)]
    pub fn record_domain_accuracy(&mut self, domain: Domain, accuracy: f32) {
        let tracker = self.domain_accuracy
            .entry(domain)
            .or_default();
        tracker.record(accuracy);

        // Check for consecutive low accuracy in this domain
        if let Some(avg) = tracker.average() {
            if avg < self.config.low_accuracy_threshold {
                tracker.consecutive_low_count += 1;
            } else {
                tracker.consecutive_low_count = 0;
            }
        }

        debug!(
            domain = ?domain,
            accuracy = accuracy,
            avg = tracker.average(),
            sample_count = tracker.sample_count(),
            "Recorded domain-specific accuracy"
        );
    }

    /// Get average accuracy for a specific domain.
    /// TASK-METAUTL-P1-001: Returns None if no samples recorded for domain.
    #[allow(dead_code)]
    pub fn get_domain_accuracy(&self, domain: Domain) -> Option<f32> {
        self.domain_accuracy.get(&domain).and_then(|t| t.average())
    }

    /// Get all domain accuracies as a HashMap.
    /// TASK-METAUTL-P1-001: For introspection and MCP exposure.
    #[allow(dead_code)]
    pub fn get_all_domain_accuracies(&self) -> HashMap<Domain, f32> {
        self.domain_accuracy
            .iter()
            .filter_map(|(domain, tracker)| {
                tracker.average().map(|avg| (*domain, avg))
            })
            .collect()
    }

    /// Get domain accuracy tracker for detailed inspection.
    /// TASK-METAUTL-P1-001: For accessing consecutive_low_count and other fields.
    #[allow(dead_code)]
    pub fn get_domain_tracker(&self, domain: Domain) -> Option<&DomainAccuracyTracker> {
        self.domain_accuracy.get(&domain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_accuracy_recording() {
        use super::super::types::Domain;

        let mut tracker = MetaUtlTracker::new();

        // Record some accuracies for different domains
        tracker.record_domain_accuracy(Domain::Code, 0.8);
        tracker.record_domain_accuracy(Domain::Code, 0.9);
        tracker.record_domain_accuracy(Domain::Medical, 0.7);

        // Verify Code domain
        let code_acc = tracker.get_domain_accuracy(Domain::Code);
        assert!(code_acc.is_some());
        let code_avg = code_acc.unwrap();
        assert!((code_avg - 0.85).abs() < 0.01, "Code accuracy should be ~0.85");

        // Verify Medical domain
        let medical_acc = tracker.get_domain_accuracy(Domain::Medical);
        assert!(medical_acc.is_some());
        assert!((medical_acc.unwrap() - 0.7).abs() < 0.01);

        // Verify untracked domain returns None
        assert!(tracker.get_domain_accuracy(Domain::Legal).is_none());
    }

    #[test]
    fn test_get_all_domain_accuracies() {
        use super::super::types::Domain;

        let mut tracker = MetaUtlTracker::new();

        tracker.record_domain_accuracy(Domain::Code, 0.8);
        tracker.record_domain_accuracy(Domain::Research, 0.6);

        let all = tracker.get_all_domain_accuracies();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key(&Domain::Code));
        assert!(all.contains_key(&Domain::Research));
    }

    #[test]
    fn test_domain_accuracy_clamping() {
        use super::super::types::Domain;

        let mut tracker = MetaUtlTracker::new();

        // Values should be clamped to [0.0, 1.0]
        tracker.record_domain_accuracy(Domain::Code, 1.5);  // Should clamp to 1.0
        tracker.record_domain_accuracy(Domain::Code, -0.5); // Should clamp to 0.0

        let avg = tracker.get_domain_accuracy(Domain::Code).unwrap();
        assert!((avg - 0.5).abs() < 0.01, "Average of clamped 1.0 and 0.0 should be 0.5");
    }

    #[test]
    fn test_domain_consecutive_low_tracking() {
        use super::super::types::Domain;

        let mut tracker = MetaUtlTracker::new();

        // Record consistently low accuracy (below default threshold of 0.7)
        for _ in 0..5 {
            tracker.record_domain_accuracy(Domain::Code, 0.5);
        }

        let domain_tracker = tracker.get_domain_tracker(Domain::Code);
        assert!(domain_tracker.is_some());
        assert!(domain_tracker.unwrap().consecutive_low_count > 0);
    }
}
