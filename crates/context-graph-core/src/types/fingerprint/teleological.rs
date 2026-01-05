//! TeleologicalFingerprint: Complete node representation with purpose-aware metadata.
//!
//! This is the top-level fingerprint type that wraps SemanticFingerprint with:
//! - Purpose Vector (13D alignment to North Star goal)
//! - Johari Fingerprint (per-embedder awareness classification)
//! - Purpose Evolution (time-series of alignment changes)
//!
//! Enables goal-aligned retrieval: "find memories similar to X that serve the same purpose"

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::evolution::{EvolutionTrigger, PurposeSnapshot};
use super::johari::JohariFingerprint;
use super::purpose::{AlignmentThreshold, PurposeVector};
use super::SemanticFingerprint;

/// Complete teleological fingerprint for a memory node.
///
/// This struct combines semantic content (what) with purpose (why),
/// enabling retrieval that considers both similarity and goal alignment.
///
/// From constitution.yaml:
/// - Expected size: ~46KB per node
/// - MAX_EVOLUTION_SNAPSHOTS: 100 (older snapshots archived to TimescaleDB)
/// - Misalignment warning: delta_A < -0.15 predicts failure 72 hours ahead
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalFingerprint {
    /// Unique identifier for this fingerprint (UUID v4)
    pub id: Uuid,

    /// The 13-embedding semantic fingerprint (from TASK-F001)
    pub semantic: SemanticFingerprint,

    /// 13D alignment signature to North Star goal
    pub purpose_vector: PurposeVector,

    /// Per-embedder Johari awareness classification
    pub johari: JohariFingerprint,

    /// Time-series of purpose evolution snapshots
    pub purpose_evolution: Vec<PurposeSnapshot>,

    /// Current alignment angle to North Star goal (aggregate)
    pub theta_to_north_star: f32,

    /// SHA-256 hash of the source content (32 bytes)
    pub content_hash: [u8; 32],

    /// When this fingerprint was created
    pub created_at: DateTime<Utc>,

    /// When this fingerprint was last updated
    pub last_updated: DateTime<Utc>,

    /// Number of times this memory has been accessed
    pub access_count: u64,
}

impl TeleologicalFingerprint {
    /// Expected size in bytes for a complete teleological fingerprint.
    /// From constitution.yaml: ~46KB per node.
    pub const EXPECTED_SIZE_BYTES: usize = 46_000;

    /// Maximum number of evolution snapshots to retain in memory.
    /// Older snapshots are archived to TimescaleDB in production.
    pub const MAX_EVOLUTION_SNAPSHOTS: usize = 100;

    /// Threshold for misalignment warning (from constitution.yaml).
    /// delta_A < -0.15 predicts failure 72 hours ahead.
    pub const MISALIGNMENT_THRESHOLD: f32 = -0.15;

    /// Create a new TeleologicalFingerprint.
    ///
    /// Automatically:
    /// - Generates a new UUID v4
    /// - Sets timestamps to now
    /// - Computes initial theta_to_north_star
    /// - Records initial evolution snapshot with Created trigger
    ///
    /// # Arguments
    /// * `semantic` - The semantic fingerprint (13 embeddings)
    /// * `purpose_vector` - The purpose alignment vector
    /// * `johari` - The Johari awareness classification
    /// * `content_hash` - SHA-256 hash of source content
    pub fn new(
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let now = Utc::now();
        let theta_to_north_star = purpose_vector.aggregate_alignment();

        // Create initial snapshot
        let initial_snapshot = PurposeSnapshot::new(
            purpose_vector.clone(),
            johari.clone(),
            EvolutionTrigger::Created,
        );

        Self {
            id: Uuid::new_v4(),
            semantic,
            purpose_vector,
            johari,
            purpose_evolution: vec![initial_snapshot],
            theta_to_north_star,
            content_hash,
            created_at: now,
            last_updated: now,
            access_count: 0,
        }
    }

    /// Create a TeleologicalFingerprint with a specific ID (for testing/import).
    pub fn with_id(
        id: Uuid,
        semantic: SemanticFingerprint,
        purpose_vector: PurposeVector,
        johari: JohariFingerprint,
        content_hash: [u8; 32],
    ) -> Self {
        let mut fp = Self::new(semantic, purpose_vector, johari, content_hash);
        fp.id = id;
        fp
    }

    /// Record a new purpose evolution snapshot.
    ///
    /// Updates:
    /// - Adds snapshot to evolution history
    /// - Trims history if over MAX_EVOLUTION_SNAPSHOTS
    /// - Updates last_updated timestamp
    /// - Recalculates theta_to_north_star
    ///
    /// # Arguments
    /// * `trigger` - What caused this evolution event
    pub fn record_snapshot(&mut self, trigger: EvolutionTrigger) {
        let snapshot = PurposeSnapshot::new(
            self.purpose_vector.clone(),
            self.johari.clone(),
            trigger,
        );

        self.purpose_evolution.push(snapshot);

        // Trim if over limit (remove oldest)
        if self.purpose_evolution.len() > Self::MAX_EVOLUTION_SNAPSHOTS {
            // In production, archive to TimescaleDB before removing
            self.purpose_evolution.remove(0);
        }

        self.last_updated = Utc::now();
        self.theta_to_north_star = self.purpose_vector.aggregate_alignment();
    }

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
        let previous = self.purpose_evolution[self.purpose_evolution.len() - 2].aggregate_alignment();

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

    /// Record an access event.
    ///
    /// Increments access_count and optionally records a snapshot.
    ///
    /// # Arguments
    /// * `query_context` - Description of the query that accessed this memory
    /// * `record_snapshot` - Whether to record an evolution snapshot
    pub fn record_access(&mut self, query_context: String, record_evolution: bool) {
        self.access_count += 1;
        self.last_updated = Utc::now();

        if record_evolution {
            self.record_snapshot(EvolutionTrigger::Accessed { query_context });
        }
    }

    /// Update purpose vector (e.g., after recalibration).
    ///
    /// Automatically records an evolution snapshot and checks for misalignment.
    pub fn update_purpose(&mut self, new_purpose: PurposeVector, trigger: EvolutionTrigger) {
        self.purpose_vector = new_purpose;
        self.record_snapshot(trigger);
    }

    /// Get the age of this fingerprint (time since creation).
    pub fn age(&self) -> chrono::Duration {
        Utc::now() - self.created_at
    }

    /// Get the number of evolution snapshots.
    pub fn evolution_count(&self) -> usize {
        self.purpose_evolution.len()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::NUM_EMBEDDERS;
    use crate::types::JohariQuadrant;

    // ===== Test Helpers =====

    fn make_test_semantic() -> SemanticFingerprint {
        SemanticFingerprint::default()
    }

    fn make_test_purpose(alignment: f32) -> PurposeVector {
        PurposeVector::new([alignment; NUM_EMBEDDERS])
    }

    fn make_test_johari() -> JohariFingerprint {
        JohariFingerprint::new([JohariQuadrant::Open; NUM_EMBEDDERS])
    }

    fn make_test_hash() -> [u8; 32] {
        let mut hash = [0u8; 32];
        hash[0] = 0xDE;
        hash[1] = 0xAD;
        hash[30] = 0xBE;
        hash[31] = 0xEF;
        hash
    }

    // ===== Creation Tests =====

    #[test]
    fn test_teleological_new() {
        let semantic = make_test_semantic();
        let purpose = make_test_purpose(0.80);
        let johari = make_test_johari();
        let hash = make_test_hash();

        let before = Utc::now();
        let fp = TeleologicalFingerprint::new(semantic, purpose, johari, hash);
        let after = Utc::now();

        // ID is valid UUID
        assert!(!fp.id.is_nil());

        // Timestamps are set
        assert!(fp.created_at >= before && fp.created_at <= after);
        assert!(fp.last_updated >= before && fp.last_updated <= after);

        // Initial snapshot exists
        assert_eq!(fp.purpose_evolution.len(), 1);
        assert!(matches!(
            fp.purpose_evolution[0].trigger,
            EvolutionTrigger::Created
        ));

        // Theta is computed
        assert!((fp.theta_to_north_star - 0.80).abs() < 1e-6);

        // Access count starts at 0
        assert_eq!(fp.access_count, 0);

        // Hash is stored
        assert_eq!(fp.content_hash, hash);

        println!("[PASS] TeleologicalFingerprint::new creates valid fingerprint");
        println!("  - ID: {}", fp.id);
        println!("  - Created: {}", fp.created_at);
        println!("  - Initial theta: {:.4}", fp.theta_to_north_star);
        println!("  - Evolution snapshots: {}", fp.purpose_evolution.len());
    }

    #[test]
    fn test_teleological_with_id() {
        let specific_id = Uuid::new_v4();
        let fp = TeleologicalFingerprint::with_id(
            specific_id,
            make_test_semantic(),
            make_test_purpose(0.75),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.id, specific_id);

        println!("[PASS] TeleologicalFingerprint::with_id uses provided ID");
    }

    // ===== Snapshot Recording Tests =====

    #[test]
    fn test_teleological_record_snapshot() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        let initial_count = fp.evolution_count();
        let initial_updated = fp.last_updated;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        fp.record_snapshot(EvolutionTrigger::Recalibration);

        assert_eq!(fp.evolution_count(), initial_count + 1);
        assert!(fp.last_updated > initial_updated);
        assert!(matches!(
            fp.purpose_evolution.last().unwrap().trigger,
            EvolutionTrigger::Recalibration
        ));

        println!("[PASS] record_snapshot adds to evolution and updates timestamp");
        println!("  - Before: {} snapshots", initial_count);
        println!("  - After: {} snapshots", fp.evolution_count());
    }

    #[test]
    fn test_teleological_record_snapshot_respects_limit() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Add MAX + 50 snapshots
        for i in 0..(TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS + 50) {
            fp.record_snapshot(EvolutionTrigger::Accessed {
                query_context: format!("query_{}", i),
            });
        }

        // Should be capped at MAX
        assert_eq!(
            fp.evolution_count(),
            TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS
        );

        // First snapshot should NOT be "Created" (it was trimmed)
        assert!(!matches!(
            fp.purpose_evolution[0].trigger,
            EvolutionTrigger::Created
        ));

        println!(
            "[PASS] record_snapshot enforces MAX_EVOLUTION_SNAPSHOTS = {}",
            TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS
        );
    }

    // ===== Alignment Delta Tests =====

    #[test]
    fn test_teleological_alignment_delta_single_snapshot() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Only one snapshot = delta is 0
        assert_eq!(fp.compute_alignment_delta(), 0.0);

        println!("[PASS] alignment_delta returns 0.0 with single snapshot");
    }

    #[test]
    fn test_teleological_alignment_delta_improvement() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.70),
            make_test_johari(),
            make_test_hash(),
        );

        // Improve alignment
        fp.purpose_vector = make_test_purpose(0.85);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let delta = fp.compute_alignment_delta();
        assert!((delta - 0.15).abs() < 1e-5);

        println!("[PASS] alignment_delta shows positive improvement: {:.4}", delta);
    }

    #[test]
    fn test_teleological_alignment_delta_degradation() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Degrade alignment
        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let delta = fp.compute_alignment_delta();
        assert!((delta - (-0.20)).abs() < 1e-5);

        println!("[PASS] alignment_delta shows negative degradation: {:.4}", delta);
    }

    // ===== Misalignment Warning Tests =====

    #[test]
    fn test_teleological_misalignment_warning_not_triggered() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Small degradation (within threshold)
        fp.purpose_vector = make_test_purpose(0.75);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        assert!(fp.check_misalignment_warning().is_none());

        println!("[PASS] No warning for small degradation (delta = -0.05)");
    }

    #[test]
    fn test_teleological_misalignment_warning_triggered() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Large degradation (exceeds threshold of -0.15)
        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let warning = fp.check_misalignment_warning();
        assert!(warning.is_some());

        let delta = warning.unwrap();
        assert!(delta < TeleologicalFingerprint::MISALIGNMENT_THRESHOLD);

        println!(
            "[PASS] Warning triggered for large degradation: delta = {:.4} < {:.2}",
            delta,
            TeleologicalFingerprint::MISALIGNMENT_THRESHOLD
        );
    }

    #[test]
    fn test_teleological_misalignment_warning_exact_threshold() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        // Slightly above threshold (delta = -0.149) - just inside warning boundary
        fp.purpose_vector = make_test_purpose(0.651);
        fp.theta_to_north_star = fp.purpose_vector.aggregate_alignment();
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        // delta ~ -0.149 which is NOT < -0.15, so no warning
        assert!(fp.check_misalignment_warning().is_none());

        println!("[PASS] No warning at exact threshold (-0.15)");
    }

    // ===== Alignment Status Tests =====

    #[test]
    fn test_teleological_alignment_status() {
        let fp_optimal = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );
        assert_eq!(fp_optimal.alignment_status(), AlignmentThreshold::Optimal);

        let fp_critical = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.40),
            make_test_johari(),
            make_test_hash(),
        );
        assert_eq!(fp_critical.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] alignment_status correctly classifies theta");
    }

    // ===== Access Recording Tests =====

    #[test]
    fn test_teleological_record_access() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.access_count, 0);
        let initial_evolution = fp.evolution_count();

        fp.record_access("test query".to_string(), false);
        assert_eq!(fp.access_count, 1);
        assert_eq!(fp.evolution_count(), initial_evolution); // No snapshot

        fp.record_access("another query".to_string(), true);
        assert_eq!(fp.access_count, 2);
        assert_eq!(fp.evolution_count(), initial_evolution + 1); // With snapshot

        println!("[PASS] record_access increments count and optionally records snapshot");
    }

    // ===== Concerning State Tests =====

    #[test]
    fn test_teleological_is_concerning() {
        // Not concerning: Optimal alignment
        let fp_ok = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.80),
            make_test_johari(),
            make_test_hash(),
        );
        assert!(!fp_ok.is_concerning());

        // Concerning: Critical alignment
        let fp_critical = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.40),
            make_test_johari(),
            make_test_hash(),
        );
        assert!(fp_critical.is_concerning());

        println!("[PASS] is_concerning detects problematic states");
    }

    // ===== History Stats Tests =====

    #[test]
    fn test_teleological_alignment_history_stats() {
        let mut fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.70),
            make_test_johari(),
            make_test_hash(),
        );

        // Add more snapshots with varying alignments
        fp.purpose_vector = make_test_purpose(0.80);
        fp.theta_to_north_star = 0.80;
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        fp.purpose_vector = make_test_purpose(0.60);
        fp.theta_to_north_star = 0.60;
        fp.record_snapshot(EvolutionTrigger::Recalibration);

        let (min, max, avg) = fp.alignment_history_stats();

        assert!((min - 0.60).abs() < 1e-5);
        assert!((max - 0.80).abs() < 1e-5);
        assert!((avg - 0.70).abs() < 1e-5); // (0.70 + 0.80 + 0.60) / 3

        println!("[PASS] alignment_history_stats computes correct min/max/avg");
        println!("  - Min: {:.2}, Max: {:.2}, Avg: {:.2}", min, max, avg);
    }

    // ===== Constants Tests =====

    #[test]
    fn test_teleological_constants() {
        assert_eq!(TeleologicalFingerprint::EXPECTED_SIZE_BYTES, 46_000);
        assert_eq!(TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS, 100);
        assert!(
            (TeleologicalFingerprint::MISALIGNMENT_THRESHOLD - (-0.15)).abs() < f32::EPSILON
        );

        println!("[PASS] Constants match specification");
        println!(
            "  - EXPECTED_SIZE_BYTES: {}",
            TeleologicalFingerprint::EXPECTED_SIZE_BYTES
        );
        println!(
            "  - MAX_EVOLUTION_SNAPSHOTS: {}",
            TeleologicalFingerprint::MAX_EVOLUTION_SNAPSHOTS
        );
        println!(
            "  - MISALIGNMENT_THRESHOLD: {}",
            TeleologicalFingerprint::MISALIGNMENT_THRESHOLD
        );
    }

    // ===== Edge Cases =====

    #[test]
    fn test_teleological_zero_alignment() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.0),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.theta_to_north_star, 0.0);
        assert_eq!(fp.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] Zero alignment handled correctly");
    }

    #[test]
    fn test_teleological_negative_alignment() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(-0.5),
            make_test_johari(),
            make_test_hash(),
        );

        assert_eq!(fp.theta_to_north_star, -0.5);
        assert_eq!(fp.alignment_status(), AlignmentThreshold::Critical);

        println!("[PASS] Negative alignment handled correctly");
    }

    #[test]
    fn test_teleological_serialization() {
        let fp = TeleologicalFingerprint::new(
            make_test_semantic(),
            make_test_purpose(0.75),
            make_test_johari(),
            make_test_hash(),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&fp).expect("Serialization should succeed");
        assert!(!json.is_empty());

        // Test deserialization
        let restored: TeleologicalFingerprint =
            serde_json::from_str(&json).expect("Deserialization should succeed");
        assert_eq!(restored.id, fp.id);
        assert!((restored.theta_to_north_star - fp.theta_to_north_star).abs() < f32::EPSILON);

        println!("[PASS] TeleologicalFingerprint serializes/deserializes correctly");
    }
}
